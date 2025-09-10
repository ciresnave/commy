/**
 * Commy FFI Header - C Interface for Multi-language SDKs
 *
 * This header file provides the C interface for the Commy distributed communication mesh.
 * It can be used to create bindings for Python, JavaScript/Node.js, Go, Java, .NET, and other languages.
 *
 * Author: Commy Team
 * Version: 0.1.0
 * License: MIT/Apache-2.0
 */

#ifndef COMMY_FFI_H
#define COMMY_FFI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C"
{
#endif

  /* ============================================================================
   * Core Types and Error Codes
   * ============================================================================ */

  /**
   * Error codes returned by Commy functions
   */
  typedef enum
  {
    COMMY_SUCCESS = 0,
    COMMY_INVALID_ARGUMENT = -1,
    COMMY_OUT_OF_MEMORY = -2,
    COMMY_NETWORK_ERROR = -3,
    COMMY_SERIALIZATION_ERROR = -4,
    COMMY_SERVICE_NOT_FOUND = -5,
    COMMY_SERVICE_ALREADY_EXISTS = -6,
    COMMY_INSTANCE_NOT_FOUND = -7,
    COMMY_INITIALIZATION_ERROR = -8,
    COMMY_CONFIGURATION_ERROR = -9,
    COMMY_HEALTH_CHECK_FAILED = -10,
    COMMY_LOAD_BALANCER_ERROR = -11,
    COMMY_TIMEOUT_ERROR = -12,
    COMMY_SECURITY_ERROR = -13,
    COMMY_PERMISSION_DENIED = -14,
    COMMY_INTERNAL_ERROR = -99
  } commy_error_t;

  /**
   * Opaque handle to a mesh coordinator instance
   */
  typedef struct
  {
    uint64_t instance_id;
  } commy_handle_t;

  /**
   * Service status enumeration
   */
  typedef enum
  {
    COMMY_SERVICE_UNKNOWN = 0,
    COMMY_SERVICE_HEALTHY = 1,
    COMMY_SERVICE_UNHEALTHY = 2,
    COMMY_SERVICE_DEGRADED = 3
  } commy_service_status_t;

  /**
   * Load balancer algorithms
   */
  typedef enum
  {
    COMMY_LB_ROUND_ROBIN = 0,
    COMMY_LB_LEAST_CONNECTIONS = 1,
    COMMY_LB_WEIGHTED_ROUND_ROBIN = 2,
    COMMY_LB_PERFORMANCE_BASED = 3,
    COMMY_LB_RANDOM = 4,
    COMMY_LB_CONSISTENT_HASH = 5
  } commy_load_balancer_algorithm_t;

  /* ============================================================================
   * Configuration Structures
   * ============================================================================ */

  /**
   * Service configuration
   */
  typedef struct
  {
    const char *service_name;
    const char *service_id;
    const char *endpoint;
    uint16_t port;
    uint32_t weight;
    const char *metadata; /* JSON string */
  } commy_service_config_t;

  /**
   * Health check configuration
   */
  typedef struct
  {
    uint64_t check_interval_ms;
    uint64_t timeout_ms;
    uint32_t max_failures;
    uint32_t recovery_checks;
  } commy_health_config_t;

  /**
   * Load balancer configuration
   */
  typedef struct
  {
    commy_load_balancer_algorithm_t algorithm;
    bool enable_circuit_breaker;
    uint32_t circuit_breaker_threshold;
    uint64_t circuit_breaker_timeout_ms;
  } commy_load_balancer_config_t;

  /**
   * Service information
   */
  typedef struct
  {
    const char *service_name;
    const char *service_id;
    const char *endpoint;
    uint16_t port;
    commy_service_status_t status;
    uint32_t weight;
    uint64_t response_time_ms;
  } commy_service_info_t;

  /**
   * Mesh statistics
   */
  typedef struct
  {
    uint32_t total_services;
    uint32_t healthy_services;
    uint32_t unhealthy_services;
    uint64_t total_requests;
    uint64_t successful_requests;
    uint64_t failed_requests;
    double average_response_time_ms;
  } commy_mesh_stats_t;

  /* ============================================================================
   * Callback Types
   * ============================================================================ */

  /**
   * Logging callback function
   * @param level Log level (0=Error, 1=Warn, 2=Info, 3=Debug)
   * @param message Log message
   */
  typedef void (*commy_log_callback_t)(int level, const char *message);

  /**
   * Health status change callback
   * @param service_id Service identifier
   * @param status New service status
   */
  typedef void (*commy_health_callback_t)(const char *service_id, commy_service_status_t status);

  /**
   * Service change callback
   * @param service_info Service information
   */
  typedef void (*commy_service_callback_t)(const commy_service_info_t *service_info);

  /* ============================================================================
   * Core Functions
   * ============================================================================ */

  /**
   * Initialize the FFI layer
   * This should be called once before using any other FFI functions
   * @return 0 on success, negative on error
   */
  int commy_ffi_init(void);

  /**
   * Cleanup the FFI layer
   * This should be called when shutting down to cleanup resources
   * @return 0 on success, negative on error
   */
  int commy_ffi_cleanup(void);

  /**
   * Get the version of the Commy library
   * @return Version string (should not be freed by caller)
   */
  const char *commy_ffi_version(void);

  /**
   * Create a new mesh coordinator instance
   * @param node_id Unique identifier for this node
   * @param listen_port Port to listen on for mesh communication
   * @return Handle to the mesh instance, or null handle on failure
   */
  commy_handle_t commy_create_mesh(const char *node_id, uint16_t listen_port);

  /**
   * Start the mesh coordinator
   * @param handle Mesh handle
   * @return 0 on success, negative error code on failure
   */
  int commy_start_mesh(commy_handle_t handle);

  /**
   * Stop the mesh coordinator
   * @param handle Mesh handle
   * @return 0 on success, negative error code on failure
   */
  int commy_stop_mesh(commy_handle_t handle);

  /**
   * Get mesh statistics
   * @param handle Mesh handle
   * @param stats Pointer to statistics structure to fill
   * @return 0 on success, negative error code on failure
   */
  int commy_get_mesh_stats(commy_handle_t handle, commy_mesh_stats_t *stats);

  /**
   * Check if mesh is running
   * @param handle Mesh handle
   * @return 1 if running, 0 if not running, negative on error
   */
  int commy_is_mesh_running(commy_handle_t handle);

  /**
   * Configure mesh settings
   * @param handle Mesh handle
   * @param health_config Health monitoring configuration (can be NULL)
   * @param lb_config Load balancer configuration (can be NULL)
   * @return 0 on success, negative error code on failure
   */
  int commy_configure_mesh(commy_handle_t handle,
                           const commy_health_config_t *health_config,
                           const commy_load_balancer_config_t *lb_config);

  /**
   * Get the node ID of the mesh
   * @param handle Mesh handle
   * @return Node ID string (must be freed with commy_free_string)
   */
  char *commy_get_node_id(commy_handle_t handle);

  /**
   * Set logging callback
   * @param callback Logging callback function
   * @return 0 on success, negative error code on failure
   */
  int commy_set_log_callback(commy_log_callback_t callback);

  /* ============================================================================
   * Service Management
   * ============================================================================ */

  /**
   * Register a service with the mesh
   * @param handle Mesh handle
   * @param config Service configuration
   * @return 0 on success, negative error code on failure
   */
  int commy_register_service(commy_handle_t handle, const commy_service_config_t *config);

  /**
   * Unregister a service from the mesh
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @return 0 on success, negative error code on failure
   */
  int commy_unregister_service(commy_handle_t handle, const char *service_id);

  /**
   * Discover services by name
   * @param handle Mesh handle
   * @param service_name Name of service to discover
   * @param services Pointer to array of service info (allocated by function)
   * @param count Pointer to number of services found
   * @return 0 on success, negative error code on failure
   * @note Use commy_free_service_info_array to free the returned array
   */
  int commy_discover_services(commy_handle_t handle,
                              const char *service_name,
                              commy_service_info_t **services,
                              size_t *count);

  /**
   * Get all registered services
   * @param handle Mesh handle
   * @param services Pointer to array of service info (allocated by function)
   * @param count Pointer to number of services
   * @return 0 on success, negative error code on failure
   * @note Use commy_free_service_info_array to free the returned array
   */
  int commy_get_all_services(commy_handle_t handle,
                             commy_service_info_t **services,
                             size_t *count);

  /**
   * Get service by ID
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param service_info Pointer to service info structure to fill
   * @return 0 on success, negative error code on failure
   */
  int commy_get_service(commy_handle_t handle,
                        const char *service_id,
                        commy_service_info_t *service_info);

  /**
   * Update service metadata
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param metadata New metadata (JSON string)
   * @return 0 on success, negative error code on failure
   */
  int commy_update_service_metadata(commy_handle_t handle,
                                    const char *service_id,
                                    const char *metadata);

  /**
   * Set service callback for notifications
   * @param handle Mesh handle
   * @param callback Service callback function
   * @return 0 on success, negative error code on failure
   */
  int commy_set_service_callback(commy_handle_t handle, commy_service_callback_t callback);

  /* ============================================================================
   * Health Monitoring
   * ============================================================================ */

  /**
   * Start health monitoring for a service
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param config Health monitoring configuration
   * @return 0 on success, negative error code on failure
   */
  int commy_start_health_monitoring(commy_handle_t handle,
                                    const char *service_id,
                                    const commy_health_config_t *config);

  /**
   * Stop health monitoring for a service
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @return 0 on success, negative error code on failure
   */
  int commy_stop_health_monitoring(commy_handle_t handle, const char *service_id);

  /**
   * Get health status of a service
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param status Pointer to status variable
   * @param response_time_ms Pointer to response time variable (can be NULL)
   * @return 0 on success, negative error code on failure
   */
  int commy_get_service_health(commy_handle_t handle,
                               const char *service_id,
                               commy_service_status_t *status,
                               uint64_t *response_time_ms);

  /**
   * Get health status of all services
   * @param handle Mesh handle
   * @param service_count Pointer to number of services
   * @param service_ids Pointer to array of service IDs (allocated by function)
   * @param statuses Pointer to array of statuses (allocated by function)
   * @return 0 on success, negative error code on failure
   * @note Use commy_free_health_status_arrays to free the returned arrays
   */
  int commy_get_all_health_status(commy_handle_t handle,
                                  size_t *service_count,
                                  char ***service_ids,
                                  commy_service_status_t **statuses);

  /**
   * Perform manual health check
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param status Pointer to status variable
   * @param response_time_ms Pointer to response time variable (can be NULL)
   * @return 0 on success, negative error code on failure
   */
  int commy_manual_health_check(commy_handle_t handle,
                                const char *service_id,
                                commy_service_status_t *status,
                                uint64_t *response_time_ms);

  /**
   * Set health callback for notifications
   * @param handle Mesh handle
   * @param callback Health callback function
   * @return 0 on success, negative error code on failure
   */
  int commy_set_health_callback(commy_handle_t handle, commy_health_callback_t callback);

  /* ============================================================================
   * Load Balancing
   * ============================================================================ */

  /**
   * Configure load balancer
   * @param handle Mesh handle
   * @param config Load balancer configuration
   * @return 0 on success, negative error code on failure
   */
  int commy_configure_load_balancer(commy_handle_t handle,
                                    const commy_load_balancer_config_t *config);

  /**
   * Select a service using load balancer
   * @param handle Mesh handle
   * @param service_name Service name to select from
   * @param client_id Client identifier (can be NULL)
   * @param selected_service Pointer to service info structure to fill
   * @return 0 on success, negative error code on failure
   */
  int commy_select_service(commy_handle_t handle,
                           const char *service_name,
                           const char *client_id,
                           commy_service_info_t *selected_service);

  /**
   * Get load balancer statistics
   * @param handle Mesh handle
   * @param service_name Service name
   * @param total_requests Pointer to total requests counter (can be NULL)
   * @param successful_requests Pointer to successful requests counter (can be NULL)
   * @param failed_requests Pointer to failed requests counter (can be NULL)
   * @param average_response_time_ms Pointer to average response time (can be NULL)
   * @return 0 on success, negative error code on failure
   */
  int commy_get_load_balancer_stats(commy_handle_t handle,
                                    const char *service_name,
                                    uint64_t *total_requests,
                                    uint64_t *successful_requests,
                                    uint64_t *failed_requests,
                                    double *average_response_time_ms);

  /**
   * Report service performance (for performance-based load balancing)
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param response_time_ms Response time in milliseconds
   * @param success Whether the request was successful
   * @return 0 on success, negative error code on failure
   */
  int commy_report_service_performance(commy_handle_t handle,
                                       const char *service_id,
                                       uint64_t response_time_ms,
                                       bool success);

  /**
   * Get circuit breaker status
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @param is_open Pointer to circuit breaker open status (can be NULL)
   * @param failure_count Pointer to failure count (can be NULL)
   * @return 0 on success, negative error code on failure
   */
  int commy_get_circuit_breaker_status(commy_handle_t handle,
                                       const char *service_id,
                                       bool *is_open,
                                       uint32_t *failure_count);

  /**
   * Reset circuit breaker
   * @param handle Mesh handle
   * @param service_id Service identifier
   * @return 0 on success, negative error code on failure
   */
  int commy_reset_circuit_breaker(commy_handle_t handle, const char *service_id);

  /**
   * Get service weights (for weighted load balancing)
   * @param handle Mesh handle
   * @param service_name Service name
   * @param service_ids Pointer to array of service IDs (allocated by function)
   * @param weights Pointer to array of weights (allocated by function)
   * @param count Pointer to number of services
   * @return 0 on success, negative error code on failure
   * @note Use commy_free_service_weights_arrays to free the returned arrays
   */
  int commy_get_service_weights(commy_handle_t handle,
                                const char *service_name,
                                char ***service_ids,
                                uint32_t **weights,
                                size_t *count);

  /* ============================================================================
   * Memory Management
   * ============================================================================ */

  /**
   * Allocate memory using the C allocator
   * @param size Size in bytes
   * @return Pointer to allocated memory, or NULL on failure
   */
  void *commy_malloc(size_t size);

  /**
   * Free memory allocated by commy_malloc
   * @param ptr Pointer to memory to free
   */
  void commy_free(void *ptr);

  /**
   * Duplicate a string
   * @param src Source string
   * @return Duplicated string (must be freed with commy_free)
   */
  char *commy_strdup(const char *src);

  /**
   * Get string length
   * @param s String
   * @return Length of string
   */
  size_t commy_strlen(const char *s);

  /**
   * Copy memory
   * @param dst Destination
   * @param src Source
   * @param size Size in bytes
   * @return Destination pointer
   */
  void *commy_memcpy(void *dst, const void *src, size_t size);

  /**
   * Set memory to a value
   * @param ptr Memory pointer
   * @param value Value to set
   * @param size Size in bytes
   * @return Memory pointer
   */
  void *commy_memset(void *ptr, int value, size_t size);

  /**
   * Free a string allocated by Commy
   * @param ptr String pointer
   */
  void commy_free_string(char *ptr);

  /**
   * Allocate an array of service info structures
   * @param count Number of structures
   * @return Pointer to array, or NULL on failure
   */
  commy_service_info_t *commy_alloc_service_info_array(size_t count);

  /**
   * Free an array of service info structures
   * @param ptr Array pointer
   * @param count Number of structures
   */
  void commy_free_service_info_array(commy_service_info_t *ptr, size_t count);

  /**
   * Free health status arrays
   * @param service_count Number of services
   * @param service_ids Service IDs array
   * @param statuses Statuses array
   */
  void commy_free_health_status_arrays(size_t service_count,
                                       char **service_ids,
                                       commy_service_status_t *statuses);

  /**
   * Free service weights arrays
   * @param count Number of services
   * @param service_ids Service IDs array
   * @param weights Weights array
   */
  void commy_free_service_weights_arrays(size_t count,
                                         char **service_ids,
                                         uint32_t *weights);

  /**
   * Initialize memory pool
   * @return 0 on success, negative on error
   */
  int commy_memory_pool_init(void);

  /**
   * Cleanup memory pool
   * @return 0 on success, negative on error
   */
  int commy_memory_pool_cleanup(void);

#ifdef __cplusplus
}
#endif

#endif /* COMMY_FFI_H */