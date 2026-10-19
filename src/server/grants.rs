//! Service-scoped access control (plan C10).
//!
//! Grants are per client, tenant and service. A tenant-wide grant is shorthand for every service of
//! the tenant. The engine is `role-system` (owner decision: reuse it, do not hand-roll permission
//! logic): one role system per tenant, resource type `service`, resource id = the service id, actions
//! `read` / `write` / `resize` / `admin`. Removing one service from a client that only holds a
//! tenant-wide grant is expressed as a deny on that service, which role-system evaluates before
//! any grant.
//!
//! Storage is role-system's in-memory storage; persistence is a later item.

use role_system::{Permission as RolePermission, Resource, Role, RoleSystem, Subject};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;

/// What a client may do to a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    Read,
    Write,
    Resize,
    Admin,
}

impl Action {
    pub const ALL: [Action; 4] = [Action::Read, Action::Write, Action::Resize, Action::Admin];

    /// The action name role-system stores.
    pub fn name(self) -> &'static str {
        match self {
            Action::Read => "read",
            Action::Write => "write",
            Action::Resize => "resize",
            Action::Admin => "admin",
        }
    }

    /// The wire permission name (`service:read`, ...).
    pub fn wire_name(self) -> &'static str {
        match self {
            Action::Read => "service:read",
            Action::Write => "service:write",
            Action::Resize => "service:resize",
            Action::Admin => "service:admin",
        }
    }

    /// Actions that satisfy a check for `self`: `admin` implies everything, `write` implies `read`.
    fn satisfied_by(self) -> &'static [Action] {
        match self {
            Action::Read => &[Action::Read, Action::Write, Action::Admin],
            Action::Write => &[Action::Write, Action::Admin],
            Action::Resize => &[Action::Resize, Action::Admin],
            Action::Admin => &[Action::Admin],
        }
    }
}

/// A permission name from the wire, parsed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spec {
    Service(Action),
    /// `tenant:admin`: administer the tenant and every service in it.
    TenantAdmin,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GrantError {
    UnknownPermission(String),
    /// A permission that cannot be given at this scope (`tenant:admin` on one service).
    WrongScope(String),
    Backend(String),
}

impl std::fmt::Display for GrantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrantError::UnknownPermission(p) => write!(f, "unknown permission '{p}'"),
            GrantError::WrongScope(p) => write!(f, "'{p}' cannot be granted at this scope"),
            GrantError::Backend(e) => write!(f, "grant store error: {e}"),
        }
    }
}

/// Parse a wire permission name.
pub fn parse_permission(name: &str) -> Result<Spec, GrantError> {
    match name {
        "service:read" => Ok(Spec::Service(Action::Read)),
        "service:write" => Ok(Spec::Service(Action::Write)),
        "service:resize" => Ok(Spec::Service(Action::Resize)),
        "service:admin" => Ok(Spec::Service(Action::Admin)),
        "tenant:admin" => Ok(Spec::TenantAdmin),
        other => Err(GrantError::UnknownPermission(other.to_string())),
    }
}

/// One client's permissions at one scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub client_id: String,
    /// `None` = tenant-wide.
    pub service_id: Option<String>,
    /// Wire names, sorted.
    pub permissions: Vec<String>,
}

/// Where grants live. The one implementation is [`RoleSystemGrants`].
pub trait GrantStore: Send + Sync {
    /// Give `client` the `specs` at a service, or tenant-wide when `service` is `None`.
    fn grant(&self, tenant: &str, client: &str, service: Option<&str>, specs: &[Spec]) -> Result<(), GrantError>;
    /// Take `specs` away (all of them when empty). Returns true if the client had access to the
    /// service (at any scope) before and has none after; for a tenant-wide revoke it returns false.
    fn revoke(&self, tenant: &str, client: &str, service: Option<&str>, specs: &[Spec]) -> Result<(), GrantError>;
    /// May `client` do `action` to `service`? Tenant-wide grants count; a deny on the service wins.
    fn allows(&self, tenant: &str, client: &str, service: &str, action: Action) -> bool;
    /// Is `client` an administrator of the tenant?
    fn is_tenant_admin(&self, tenant: &str, client: &str) -> bool;
    /// List grants, optionally of one client and/or one service.
    fn list(&self, tenant: &str, client: Option<&str>, service: Option<&str>) -> Vec<Entry>;
    /// Import what authentication gave a session as tenant-wide grants, so every service check
    /// goes through this one engine. Additive and idempotent.
    fn import_session(
        &self,
        tenant: &str,
        client: &str,
        perms: &crate::auth::PermissionSet,
    ) -> Result<(), GrantError>;
}

const ALLOW: &str = "allow";
const DENY: &str = "deny";
const TENANT_RESOURCE: &str = "tenant";
const SERVICE_RESOURCE: &str = "service";

fn allow_role(client: &str, scope: &str) -> String {
    format!("{ALLOW}|{scope}|{client}")
}

fn deny_role(client: &str) -> String {
    format!("{DENY}||{client}")
}

/// [`GrantStore`] on `role-system`, in-memory.
#[derive(Default)]
pub struct RoleSystemGrants {
    systems: Mutex<HashMap<String, RoleSystem>>,
}

impl RoleSystemGrants {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<R>(&self, tenant: &str, f: impl FnOnce(&mut RoleSystem) -> R) -> R {
        let mut g = self.systems.lock().unwrap_or_else(|e| e.into_inner());
        f(g.entry(tenant.to_string()).or_default())
    }

}

impl RoleSystemGrants {
    fn import_impl(
        &self,
        tenant: &str,
        client: &str,
        perms: &crate::auth::PermissionSet,
    ) -> Result<(), GrantError> {
        use crate::auth::Permission as P;
        let mut specs = Vec::new();
        if perms.has_permission(&P::TenantAdmin) {
            specs.push(Spec::TenantAdmin);
        }
        for (p, a) in [
            (P::ServiceRead, Action::Read),
            (P::ServiceWrite, Action::Write),
            (P::ServiceAdmin, Action::Admin),
            (P::ServiceResize, Action::Resize),
        ] {
            if perms.has_permission(&p) {
                specs.push(Spec::Service(a));
            }
        }
        if specs.is_empty() {
            return Ok(());
        }
        self.grant(tenant, client, None, &specs)
    }
}

fn backend<E: std::fmt::Display>(e: E) -> GrantError {
    GrantError::Backend(e.to_string())
}

/// Permissions for a spec at a scope.
fn role_permissions(spec: Spec, service: Option<&str>) -> Vec<RolePermission> {
    match (spec, service) {
        (Spec::Service(a), Some(s)) => vec![RolePermission::with_instance(a.name(), SERVICE_RESOURCE, s)],
        (Spec::Service(a), None) => vec![RolePermission::new(a.name(), SERVICE_RESOURCE)],
        // Tenant admin: administer the tenant resource and every service in it.
        (Spec::TenantAdmin, _) => vec![
            RolePermission::new("admin", TENANT_RESOURCE),
            RolePermission::new("admin", SERVICE_RESOURCE),
        ],
    }
}

impl GrantStore for RoleSystemGrants {
    fn import_session(
        &self,
        tenant: &str,
        client: &str,
        perms: &crate::auth::PermissionSet,
    ) -> Result<(), GrantError> {
        self.import_impl(tenant, client, perms)
    }

    fn grant(&self, tenant: &str, client: &str, service: Option<&str>, specs: &[Spec]) -> Result<(), GrantError> {
        if specs.is_empty() {
            return Ok(());
        }
        if service.is_some() {
            if let Some(Spec::TenantAdmin) = specs.iter().find(|s| matches!(s, Spec::TenantAdmin)) {
                return Err(GrantError::WrongScope("tenant:admin".to_string()));
            }
        }
        self.with(tenant, |sys| {
            let subject = Subject::user(client);
            let name = allow_role(client, service.unwrap_or("*"));
            let mut role = sys.get_role(&name).map_err(backend)?.unwrap_or_else(|| Role::new(name.clone()));
            let existed = sys.get_role(&name).map_err(backend)?.is_some();
            for spec in specs {
                for p in role_permissions(*spec, service) {
                    role = role.add_permission(p);
                }
            }
            if existed {
                sys.update_role(role).map_err(backend)?;
            } else {
                sys.register_role(role).map_err(backend)?;
            }
            sys.assign_role(&subject, &name).map_err(backend)?;

            // A service-scoped grant lifts an earlier deny of the same action on that service.
            if let Some(service) = service {
                let dname = deny_role(client);
                if let Some(mut drole) = sys.get_role(&dname).map_err(backend)? {
                    for spec in specs {
                        if let Spec::Service(a) = spec {
                            drole.remove_permission(&RolePermission::deny_instance(a.name(), SERVICE_RESOURCE, service));
                        }
                    }
                    sys.update_role(drole).map_err(backend)?;
                }
            }
            Ok(())
        })
    }

    fn revoke(&self, tenant: &str, client: &str, service: Option<&str>, specs: &[Spec]) -> Result<(), GrantError> {
        // Empty means everything at that scope.
        let all: Vec<Spec> = if specs.is_empty() {
            let mut v: Vec<Spec> = Action::ALL.iter().map(|a| Spec::Service(*a)).collect();
            if service.is_none() {
                v.push(Spec::TenantAdmin);
            }
            v
        } else {
            specs.to_vec()
        };
        if service.is_some() && all.iter().any(|s| matches!(s, Spec::TenantAdmin)) {
            return Err(GrantError::WrongScope("tenant:admin".to_string()));
        }
        self.with(tenant, |sys| {
            let name = allow_role(client, service.unwrap_or("*"));
            if let Some(mut role) = sys.get_role(&name).map_err(backend)? {
                for spec in &all {
                    for p in role_permissions(*spec, service) {
                        role.remove_permission(&p);
                    }
                }
                sys.update_role(role).map_err(backend)?;
            }
            // Revoking one service from a client that still reaches it through a tenant-wide grant
            // (or through another action it keeps) needs an explicit deny.
            if let Some(service) = service {
                let dname = deny_role(client);
                let mut drole = sys
                    .get_role(&dname)
                    .map_err(backend)?
                    .unwrap_or_else(|| Role::new(dname.clone()));
                let existed = sys.get_role(&dname).map_err(backend)?.is_some();
                for spec in &all {
                    if let Spec::Service(a) = spec {
                        drole = drole.add_permission(RolePermission::deny_instance(a.name(), SERVICE_RESOURCE, service));
                    }
                }
                if existed {
                    sys.update_role(drole).map_err(backend)?;
                } else {
                    sys.register_role(drole).map_err(backend)?;
                }
                sys.assign_role(&Subject::user(client), &dname).map_err(backend)?;
            }
            Ok(())
        })
    }

    fn allows(&self, tenant: &str, client: &str, service: &str, action: Action) -> bool {
        self.with(tenant, |sys| {
            let subject = Subject::user(client);
            let resource = Resource::new(service, SERVICE_RESOURCE);
            action
                .satisfied_by()
                .iter()
                .any(|a| sys.check_permission(&subject, a.name(), &resource).unwrap_or(false))
        })
    }

    fn is_tenant_admin(&self, tenant: &str, client: &str) -> bool {
        self.with(tenant, |sys| {
            sys.check_permission(&Subject::user(client), "admin", &Resource::new(tenant, TENANT_RESOURCE))
                .unwrap_or(false)
        })
    }

    fn list(&self, tenant: &str, client: Option<&str>, service: Option<&str>) -> Vec<Entry> {
        self.with(tenant, |sys| {
            // (client, scope) -> wire names
            let mut found: BTreeMap<(String, Option<String>), BTreeSet<String>> = BTreeMap::new();
            let subjects: Vec<(String, Vec<String>)> = sys
                .subject_roles()
                .iter()
                .map(|e| (e.key().clone(), e.value().iter().cloned().collect()))
                .collect();
            for (_subject, role_names) in subjects {
                for rn in role_names {
                    let mut parts = rn.splitn(3, '|');
                    let (kind, scope, who) = (parts.next(), parts.next(), parts.next());
                    let (Some(ALLOW), Some(scope), Some(who)) = (kind, scope, who) else {
                        continue;
                    };
                    if client.is_some_and(|c| c != who) {
                        continue;
                    }
                    let scope = (scope != "*").then(|| scope.to_string());
                    if service.is_some() && scope.is_some() && scope.as_deref() != service {
                        continue;
                    }
                    let Ok(Some(role)) = sys.get_role(&rn) else { continue };
                    let names: BTreeSet<String> = role
                        .permissions()
                        .permissions()
                        .iter()
                        .filter_map(|p| match (p.resource_type(), p.action()) {
                            (TENANT_RESOURCE, "admin") => Some("tenant:admin".to_string()),
                            (SERVICE_RESOURCE, "read") => Some(Action::Read.wire_name().to_string()),
                            (SERVICE_RESOURCE, "write") => Some(Action::Write.wire_name().to_string()),
                            (SERVICE_RESOURCE, "resize") => Some(Action::Resize.wire_name().to_string()),
                            (SERVICE_RESOURCE, "admin") if scope.is_some() => {
                                Some(Action::Admin.wire_name().to_string())
                            }
                            (SERVICE_RESOURCE, "admin") => Some(Action::Admin.wire_name().to_string()),
                            _ => None,
                        })
                        .collect();
                    if names.is_empty() {
                        continue;
                    }
                    found.entry((who.to_string(), scope)).or_default().extend(names);
                }
            }
            found
                .into_iter()
                .map(|((client_id, service_id), perms)| Entry {
                    client_id,
                    service_id,
                    permissions: perms.into_iter().collect(),
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> RoleSystemGrants {
        RoleSystemGrants::new()
    }

    #[test]
    fn nothing_is_allowed_without_a_grant() {
        let g = store();
        for a in Action::ALL {
            assert!(!g.allows("t", "bob", "svc-1", a), "{a:?}");
        }
        assert!(!g.is_tenant_admin("t", "bob"));
    }

    #[test]
    fn a_service_grant_reaches_that_service_only() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Write)]).unwrap();
        assert!(g.allows("t", "bob", "svc-1", Action::Write));
        assert!(g.allows("t", "bob", "svc-1", Action::Read), "write implies read");
        assert!(!g.allows("t", "bob", "svc-1", Action::Admin));
        assert!(!g.allows("t", "bob", "svc-1", Action::Resize));
        assert!(!g.allows("t", "bob", "svc-2", Action::Read), "another service is out of reach");
        assert!(!g.allows("t", "carol", "svc-1", Action::Read), "another client is out of reach");
        assert!(!g.allows("other-tenant", "bob", "svc-1", Action::Read), "another tenant is out of reach");
    }

    #[test]
    fn a_tenant_wide_grant_is_shorthand_for_every_service() {
        let g = store();
        g.grant("t", "bob", None, &[Spec::Service(Action::Read)]).unwrap();
        assert!(g.allows("t", "bob", "svc-1", Action::Read));
        assert!(g.allows("t", "bob", "svc-99", Action::Read));
        assert!(!g.allows("t", "bob", "svc-1", Action::Write));
        assert!(!g.allows("u", "bob", "svc-1", Action::Read));
    }

    #[test]
    fn admin_implies_write_read_and_resize() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Admin)]).unwrap();
        for a in Action::ALL {
            assert!(g.allows("t", "bob", "svc-1", a), "{a:?}");
        }
    }

    #[test]
    fn resize_is_separate_from_write() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Resize)]).unwrap();
        assert!(g.allows("t", "bob", "svc-1", Action::Resize));
        assert!(!g.allows("t", "bob", "svc-1", Action::Write));
        assert!(!g.allows("t", "bob", "svc-1", Action::Read));
    }

    #[test]
    fn tenant_admin_reaches_every_service_and_the_tenant() {
        let g = store();
        g.grant("t", "root", None, &[Spec::TenantAdmin]).unwrap();
        assert!(g.is_tenant_admin("t", "root"));
        assert!(g.allows("t", "root", "svc-1", Action::Admin));
        assert!(!g.is_tenant_admin("u", "root"));
        assert!(!g.is_tenant_admin("t", "bob"));
    }

    #[test]
    fn tenant_admin_cannot_be_granted_on_one_service() {
        let g = store();
        let e = g.grant("t", "bob", Some("svc-1"), &[Spec::TenantAdmin]).unwrap_err();
        assert!(matches!(e, GrantError::WrongScope(_)), "{e:?}");
        assert!(!g.allows("t", "bob", "svc-1", Action::Read));
    }

    #[test]
    fn revoking_a_service_grant_removes_access_to_that_service_only() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Write)]).unwrap();
        g.grant("t", "bob", Some("svc-2"), &[Spec::Service(Action::Write)]).unwrap();
        g.revoke("t", "bob", Some("svc-1"), &[]).unwrap();
        assert!(!g.allows("t", "bob", "svc-1", Action::Read));
        assert!(g.allows("t", "bob", "svc-2", Action::Write));
    }

    #[test]
    fn revoking_a_service_beats_a_tenant_wide_grant() {
        let g = store();
        g.grant("t", "bob", None, &[Spec::Service(Action::Write)]).unwrap();
        g.revoke("t", "bob", Some("svc-1"), &[]).unwrap();
        assert!(!g.allows("t", "bob", "svc-1", Action::Read), "denied on svc-1");
        assert!(!g.allows("t", "bob", "svc-1", Action::Write));
        assert!(g.allows("t", "bob", "svc-2", Action::Write), "still reaches the other services");
    }

    #[test]
    fn revoking_a_service_beats_tenant_admin_too() {
        let g = store();
        g.grant("t", "root", None, &[Spec::TenantAdmin]).unwrap();
        g.revoke("t", "root", Some("svc-1"), &[]).unwrap();
        assert!(!g.allows("t", "root", "svc-1", Action::Read));
        assert!(g.allows("t", "root", "svc-2", Action::Admin));
    }

    #[test]
    fn a_new_service_grant_lifts_an_earlier_revoke() {
        let g = store();
        g.grant("t", "bob", None, &[Spec::Service(Action::Read)]).unwrap();
        g.revoke("t", "bob", Some("svc-1"), &[]).unwrap();
        assert!(!g.allows("t", "bob", "svc-1", Action::Read));
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Read)]).unwrap();
        assert!(g.allows("t", "bob", "svc-1", Action::Read));
    }

    #[test]
    fn revoking_one_permission_keeps_the_rest() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Write), Spec::Service(Action::Resize)]).unwrap();
        g.revoke("t", "bob", Some("svc-1"), &[Spec::Service(Action::Write)]).unwrap();
        assert!(!g.allows("t", "bob", "svc-1", Action::Write));
        assert!(g.allows("t", "bob", "svc-1", Action::Resize));
    }

    #[test]
    fn a_tenant_wide_revoke_removes_the_tenant_wide_grant() {
        let g = store();
        g.grant("t", "bob", None, &[Spec::Service(Action::Read)]).unwrap();
        g.revoke("t", "bob", None, &[]).unwrap();
        assert!(!g.allows("t", "bob", "svc-1", Action::Read));
    }

    #[test]
    fn unknown_permission_names_are_refused() {
        assert_eq!(parse_permission("service:write"), Ok(Spec::Service(Action::Write)));
        assert_eq!(parse_permission("tenant:admin"), Ok(Spec::TenantAdmin));
        for bad in ["", "read", "service:delete", "SERVICE:READ", "tenant:read"] {
            assert!(matches!(parse_permission(bad), Err(GrantError::UnknownPermission(_))), "{bad}");
        }
    }

    #[test]
    fn list_shows_each_scope_and_filters_by_client_and_service() {
        let g = store();
        g.grant("t", "bob", Some("svc-1"), &[Spec::Service(Action::Write)]).unwrap();
        g.grant("t", "bob", None, &[Spec::Service(Action::Read)]).unwrap();
        g.grant("t", "carol", Some("svc-2"), &[Spec::Service(Action::Admin)]).unwrap();
        let all = g.list("t", None, None);
        assert_eq!(all.len(), 3, "{all:?}");
        assert!(all.contains(&Entry { client_id: "bob".into(), service_id: Some("svc-1".into()), permissions: vec!["service:write".into()] }));
        assert!(all.contains(&Entry { client_id: "bob".into(), service_id: None, permissions: vec!["service:read".into()] }));
        let bob = g.list("t", Some("bob"), None);
        assert_eq!(bob.len(), 2);
        // Listing one service shows its own grants and the tenant-wide ones that also reach it.
        let svc2 = g.list("t", None, Some("svc-2"));
        assert_eq!(svc2.len(), 2, "{svc2:?}");
        assert!(svc2.contains(&Entry { client_id: "carol".into(), service_id: Some("svc-2".into()), permissions: vec!["service:admin".into()] }));
        assert!(svc2.contains(&Entry { client_id: "bob".into(), service_id: None, permissions: vec!["service:read".into()] }));
        assert!(g.list("other", None, None).is_empty());
    }

    #[test]
    fn session_permissions_are_imported_as_tenant_wide_grants() {
        use crate::auth::{Permission as P, PermissionSet};
        let g = store();
        g.import_session("t", "dev", &PermissionSet::admin()).unwrap();
        assert!(g.is_tenant_admin("t", "dev"));
        assert!(g.allows("t", "dev", "svc-1", Action::Admin));
        let mut ro = PermissionSet::new();
        ro.grant(P::ServiceRead);
        g.import_session("t", "viewer", &ro).unwrap();
        assert!(g.allows("t", "viewer", "svc-1", Action::Read));
        assert!(!g.allows("t", "viewer", "svc-1", Action::Write));
        assert!(!g.is_tenant_admin("t", "viewer"));
    }
}
