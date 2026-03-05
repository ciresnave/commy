//! Permission management for multi-tenant access control
//! 
//! This module defines the permission model for COMMY:
//! - Per-(Client, Tenant) permission tracking
//! - Fine-grained resource-level permissions
//! - Permission inheritance and delegation

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A granular permission in COMMY
/// 
/// Permissions are hierarchical and resource-specific
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Permission {
    /// Tenant-level permissions
    TenantAdmin,
    TenantRead,
    TenantWrite,
    
    /// Service-level permissions
    ServiceCreate,
    ServiceDelete,
    ServiceAdmin,
    ServiceRead,
    ServiceWrite,
    
    /// Variable-level permissions
    VariableRead,
    VariableWrite,
    VariableDelete,
    
    /// Client management permissions
    ClientKick,
    ClientPermissionGrant,
    ClientPermissionRevoke,
    
    /// Custom permission
    Custom(String),
}

/// A set of permissions for a client within a tenant
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PermissionSet {
    permissions: HashSet<Permission>,
}

impl PermissionSet {
    /// Create a new empty permission set
    pub fn new() -> Self {
        Self {
            permissions: HashSet::new(),
        }
    }

    /// Create a permission set with read-only access
    pub fn read_only() -> Self {
        let mut set = Self::new();
        set.grant(Permission::TenantRead);
        set.grant(Permission::ServiceRead);
        set.grant(Permission::VariableRead);
        set
    }

    /// Create a permission set with read-write access
    pub fn read_write() -> Self {
        let mut set = Self::read_only();
        set.grant(Permission::TenantWrite);
        set.grant(Permission::ServiceWrite);
        set.grant(Permission::VariableWrite);
        set
    }

    /// Create a permission set with full admin access
    pub fn admin() -> Self {
        let mut set = Self::new();
        set.grant(Permission::TenantAdmin);
        set.grant(Permission::ServiceAdmin);
        set.grant(Permission::ClientKick);
        set.grant(Permission::ClientPermissionGrant);
        set.grant(Permission::ClientPermissionRevoke);
        set
    }

    /// Grant a permission
    pub fn grant(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    /// Revoke a permission
    pub fn revoke(&mut self, permission: &Permission) {
        self.permissions.remove(permission);
    }

    /// Check if a permission is granted
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Check if any of multiple permissions are granted
    pub fn has_any(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    /// Check if all of multiple permissions are granted
    pub fn has_all(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }

    /// Get all permissions
    pub fn all_permissions(&self) -> Vec<Permission> {
        self.permissions.iter().cloned().collect()
    }

    /// Check if this permission set has admin privileges
    pub fn is_admin(&self) -> bool {
        self.has_permission(&Permission::TenantAdmin)
    }
    
    /// Check if permission set is empty
    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }
    
    /// Compute intersection of two permission sets
    /// Returns a new set containing only permissions present in both
    pub fn intersection(&self, other: &PermissionSet) -> PermissionSet {
        let intersected: std::collections::HashSet<_> = self.permissions
            .intersection(&other.permissions)
            .cloned()
            .collect();
        
        PermissionSet {
            permissions: intersected,
        }
    }
    
    /// Compute union of two permission sets
    /// Returns a new set containing permissions from either set
    pub fn union(&self, other: &PermissionSet) -> PermissionSet {
        let unioned: std::collections::HashSet<_> = self.permissions
            .union(&other.permissions)
            .cloned()
            .collect();
        
        PermissionSet {
            permissions: unioned,
        }
    }
    
    /// Create a PermissionSet from auth-framework scopes
    pub fn from_scopes(scopes: &[String]) -> Self {
        let mut set = Self::new();
        
        for scope in scopes {
            match scope.as_str() {
                "read" => {
                    set.grant(Permission::TenantRead);
                    set.grant(Permission::ServiceRead);
                    set.grant(Permission::VariableRead);
                }
                "write" => {
                    set.grant(Permission::TenantWrite);
                    set.grant(Permission::ServiceWrite);
                    set.grant(Permission::VariableWrite);
                }
                "admin" => {
                    set.grant(Permission::TenantAdmin);
                    set.grant(Permission::ServiceAdmin);
                    set.grant(Permission::ClientKick);
                    set.grant(Permission::ClientPermissionGrant);
                    set.grant(Permission::ClientPermissionRevoke);
                }
                "tenant:read" => set.grant(Permission::TenantRead),
                "tenant:write" => set.grant(Permission::TenantWrite),
                "tenant:admin" => set.grant(Permission::TenantAdmin),
                "service:create" => set.grant(Permission::ServiceCreate),
                "service:delete" => set.grant(Permission::ServiceDelete),
                "service:admin" => set.grant(Permission::ServiceAdmin),
                "service:read" => set.grant(Permission::ServiceRead),
                "service:write" => set.grant(Permission::ServiceWrite),
                "variable:read" => set.grant(Permission::VariableRead),
                "variable:write" => set.grant(Permission::VariableWrite),
                "variable:delete" => set.grant(Permission::VariableDelete),
                "client:kick" => set.grant(Permission::ClientKick),
                "client:grant" => set.grant(Permission::ClientPermissionGrant),
                "client:revoke" => set.grant(Permission::ClientPermissionRevoke),
                custom => set.grant(Permission::Custom(custom.to_string())),
            }
        }
        
        set
    }
}

/// Per-(Client, Tenant) permission record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientTenantPermissions {
    /// Client ID
    pub client_id: String,
    
    /// Tenant ID
    pub tenant_id: String,
    
    /// Permission set for this client in this tenant
    pub permissions: PermissionSet,
    
    /// When these permissions were granted
    pub granted_at: String,
    
    /// When these permissions expire (if any)
    pub expires_at: Option<String>,
}

impl ClientTenantPermissions {
    /// Create new client-tenant permissions
    pub fn new(client_id: String, tenant_id: String, permissions: PermissionSet) -> Self {
        Self {
            client_id,
            tenant_id,
            permissions,
            granted_at: chrono::Utc::now().to_rfc3339(),
            expires_at: None,
        }
    }

    /// Check if permissions have expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = &self.expires_at {
            if let Ok(expiry) = chrono::DateTime::parse_from_rfc3339(expires) {
                return chrono::Utc::now() > expiry.with_timezone(&chrono::Utc);
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_set_hierarchy() {
        let mut perms = PermissionSet::new();
        
        // Initially empty
        assert!(!perms.has_permission(&Permission::TenantRead));
        
        // Grant permission
        perms.grant(Permission::TenantRead);
        assert!(perms.has_permission(&Permission::TenantRead));
        
        // Revoke permission
        perms.revoke(&Permission::TenantRead);
        assert!(!perms.has_permission(&Permission::TenantRead));
    }

    #[test]
    fn test_predefined_permission_sets() {
        let read_only = PermissionSet::read_only();
        assert!(read_only.has_permission(&Permission::TenantRead));
        assert!(!read_only.has_permission(&Permission::TenantWrite));
        
        let read_write = PermissionSet::read_write();
        assert!(read_write.has_permission(&Permission::TenantRead));
        assert!(read_write.has_permission(&Permission::TenantWrite));
        
        let admin = PermissionSet::admin();
        assert!(admin.is_admin());
        assert!(admin.has_permission(&Permission::ClientKick));
    }

    #[test]
    fn test_has_any_has_all() {
        let mut perms = PermissionSet::new();
        perms.grant(Permission::TenantRead);
        perms.grant(Permission::VariableRead);
        
        assert!(perms.has_any(&[Permission::TenantRead, Permission::TenantWrite]));
        assert!(!perms.has_all(&[Permission::TenantRead, Permission::TenantWrite]));
    }

    #[test]
    fn test_is_empty_and_all_permissions() {
        let empty = PermissionSet::new();
        assert!(empty.is_empty());
        assert!(empty.all_permissions().is_empty());

        let rw = PermissionSet::read_write();
        assert!(!rw.is_empty());
        assert!(!rw.all_permissions().is_empty());
    }

    #[test]
    fn test_is_admin() {
        let admin = PermissionSet::admin();
        assert!(admin.is_admin());

        let read_only = PermissionSet::read_only();
        assert!(!read_only.is_admin());
    }

    #[test]
    fn test_intersection() {
        let mut a = PermissionSet::new();
        a.grant(Permission::TenantRead);
        a.grant(Permission::VariableRead);

        let mut b = PermissionSet::new();
        b.grant(Permission::TenantRead);
        b.grant(Permission::ServiceWrite);

        let intersect = a.intersection(&b);
        assert!(intersect.has_permission(&Permission::TenantRead));
        assert!(!intersect.has_permission(&Permission::VariableRead));
        assert!(!intersect.has_permission(&Permission::ServiceWrite));
    }

    #[test]
    fn test_union() {
        let mut a = PermissionSet::new();
        a.grant(Permission::TenantRead);

        let mut b = PermissionSet::new();
        b.grant(Permission::ServiceWrite);

        let union = a.union(&b);
        assert!(union.has_permission(&Permission::TenantRead));
        assert!(union.has_permission(&Permission::ServiceWrite));
    }

    #[test]
    fn test_from_scopes_read() {
        let scopes = vec!["read".to_string()];
        let perms = PermissionSet::from_scopes(&scopes);
        assert!(perms.has_permission(&Permission::TenantRead));
        assert!(perms.has_permission(&Permission::ServiceRead));
        assert!(perms.has_permission(&Permission::VariableRead));
        assert!(!perms.has_permission(&Permission::TenantWrite));
    }

    #[test]
    fn test_from_scopes_write() {
        let scopes = vec!["write".to_string()];
        let perms = PermissionSet::from_scopes(&scopes);
        assert!(perms.has_permission(&Permission::TenantWrite));
        assert!(perms.has_permission(&Permission::VariableWrite));
    }

    #[test]
    fn test_from_scopes_admin() {
        let scopes = vec!["admin".to_string()];
        let perms = PermissionSet::from_scopes(&scopes);
        assert!(perms.is_admin());
        assert!(perms.has_permission(&Permission::ClientKick));
    }

    #[test]
    fn test_from_scopes_granular() {
        let scopes = vec![
            "service:create".to_string(),
            "variable:delete".to_string(),
            "client:kick".to_string(),
        ];
        let perms = PermissionSet::from_scopes(&scopes);
        assert!(perms.has_permission(&Permission::ServiceCreate));
        assert!(perms.has_permission(&Permission::VariableDelete));
        assert!(perms.has_permission(&Permission::ClientKick));
        assert!(!perms.has_permission(&Permission::TenantRead));
    }

    #[test]
    fn test_from_scopes_custom() {
        let scopes = vec!["my_custom_scope".to_string()];
        let perms = PermissionSet::from_scopes(&scopes);
        assert!(perms.has_permission(&Permission::Custom("my_custom_scope".to_string())));
    }

    #[test]
    fn test_client_tenant_permissions_is_expired_false_when_no_expiry() {
        let perms = PermissionSet::read_only();
        let ctp = ClientTenantPermissions::new(
            "client1".to_string(),
            "tenant1".to_string(),
            perms,
        );
        assert!(!ctp.is_expired());
    }

    #[test]
    fn test_client_tenant_permissions_is_expired_true_for_past_date() {
        let perms = PermissionSet::new();
        let mut ctp = ClientTenantPermissions::new(
            "c".to_string(),
            "t".to_string(),
            perms,
        );
        // Set expiry to a past date
        ctp.expires_at = Some("2000-01-01T00:00:00+00:00".to_string());
        assert!(ctp.is_expired());
    }
}
