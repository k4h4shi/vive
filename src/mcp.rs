//! MCP (Model Context Protocol) server module for Vive.
//!
//! This module implements an MCP server that exposes Vive's internal state
//! to external tools (like Claude Code) for monitoring and management.
//!
//! ## Resources
//!
//! The server exposes the following resources:
//!
//! - `vive://projects` - All projects and their worktrees
//! - `vive://status` - Agent statuses for all sessions
//! - `vive://logs/{session_id}` - Pane preview for a specific tmux target
//!
//! ## Usage
//!
//! The MCP server runs as a separate binary that communicates via stdio.
//! It can be started using `vive --mcp-server` and configured in Claude Desktop
//! or other MCP clients.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rmcp::Error as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    AnnotateAble, Implementation, ListResourceTemplatesResult, ListResourcesResult,
    PaginatedRequestParam, RawResource, RawResourceTemplate, ReadResourceRequestParam,
    ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::{RequestContext, RoleServer, ServiceExt};
use serde::Serialize;
use tokio::io::{stdin, stdout};

use crate::discovery::{Project, Worktree};
use crate::state::AgentStatus;

/// A snapshot of Vive's state for MCP resource access.
///
/// This struct provides a serializable view of the application state
/// that can be safely shared with the MCP server.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ViveStateSnapshot {
    /// All discovered projects with their worktrees.
    pub projects: Vec<ProjectSnapshot>,
    /// Agent statuses keyed by session ID.
    pub statuses: HashMap<String, AgentStatusSnapshot>,
    /// Pane preview contents keyed by session ID.
    pub pane_previews: HashMap<String, String>,
}

/// A serializable snapshot of a project.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectSnapshot {
    /// Project name.
    pub name: String,
    /// Project path.
    pub path: String,
    /// Worktrees in this project.
    pub worktrees: Vec<WorktreeSnapshot>,
}

/// A serializable snapshot of a worktree.
#[derive(Debug, Clone, Serialize)]
pub struct WorktreeSnapshot {
    /// Worktree path.
    pub path: String,
    /// Git commit hash.
    pub commit: String,
    /// Branch name (None for detached HEAD).
    pub branch: Option<String>,
    /// Tmux target for this worktree (session:window).
    pub tmux_target: Option<String>,
}

/// A serializable snapshot of agent status.
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatusSnapshot {
    /// Status type (e.g., "Working", "Idle", "Error").
    pub status: String,
    /// Status icon.
    pub icon: String,
    /// Detailed status text.
    pub status_text: String,
}

impl From<&Project> for ProjectSnapshot {
    fn from(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            path: project.path.to_string_lossy().to_string(),
            worktrees: project
                .worktrees
                .iter()
                .map(|wt| WorktreeSnapshot::from_worktree(wt, &project.name))
                .collect(),
        }
    }
}

impl WorktreeSnapshot {
    fn from_worktree(worktree: &Worktree, project_name: &str) -> Self {
        Self {
            path: worktree.path.to_string_lossy().to_string(),
            commit: worktree.commit.clone(),
            branch: worktree.branch.clone(),
            tmux_target: worktree.tmux_target(project_name),
        }
    }
}

impl From<&AgentStatus> for AgentStatusSnapshot {
    fn from(status: &AgentStatus) -> Self {
        let status_name = match status {
            AgentStatus::Working { .. } => "Working",
            AgentStatus::WaitingEdit { .. } => "WaitingEdit",
            AgentStatus::WaitingShell { .. } => "WaitingShell",
            AgentStatus::WaitingOther => "WaitingOther",
            AgentStatus::Idle => "Idle",
            AgentStatus::Success => "Success",
            AgentStatus::Error => "Error",
        };

        Self {
            status: status_name.to_string(),
            icon: status.icon().to_string(),
            status_text: status.status_text(),
        }
    }
}

/// Shared state provider for the MCP server.
///
/// This uses `Arc<RwLock<ViveStateSnapshot>>` to allow the TUI thread
/// to update the state while the MCP server reads it.
pub type SharedState = Arc<RwLock<ViveStateSnapshot>>;

/// The Vive MCP server handler.
///
/// Implements the `ServerHandler` trait to respond to MCP resource requests.
#[derive(Clone)]
pub struct ViveMcpServer {
    /// Shared state snapshot.
    state: SharedState,
}

impl ViveMcpServer {
    /// Creates a new MCP server with the given shared state.
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    /// Gets the current state snapshot.
    fn get_state(&self) -> ViveStateSnapshot {
        self.state.read().unwrap().clone()
    }
}

impl ServerHandler for ViveMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                resources: Some(rmcp::model::ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "vive".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: None,
        }
    }

    async fn list_resources(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = vec![
            RawResource {
                uri: "vive://projects".to_string(),
                name: "Projects".to_string(),
                description: Some(
                    "All projects and worktrees (tasks) discovered by Vive".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
                size: None,
            }
            .no_annotation(),
            RawResource {
                uri: "vive://status".to_string(),
                name: "Status".to_string(),
                description: Some("Agent statuses for all sessions (project:branch)".to_string()),
                mime_type: Some("application/json".to_string()),
                size: None,
            }
            .no_annotation(),
        ];

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let templates = vec![RawResourceTemplate {
            uri_template: "vive://logs/{session_id}".to_string(),
            name: "Session Logs".to_string(),
            description: Some(
                "Pane preview content for a specific tmux target. Use session_id in format 'project:branch'".to_string(),
            ),
            mime_type: Some("text/plain".to_string()),
        }
        .no_annotation()];

        Ok(ListResourceTemplatesResult {
            resource_templates: templates,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        let state = self.get_state();

        let content = if uri == "vive://projects" {
            // Return all projects and worktrees as JSON
            serde_json::to_string_pretty(&state.projects).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize projects: {e}"), None)
            })?
        } else if uri == "vive://status" {
            // Return all agent statuses as JSON
            serde_json::to_string_pretty(&state.statuses).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize statuses: {e}"), None)
            })?
        } else if let Some(session_id) = uri.strip_prefix("vive://logs/") {
            // Return pane preview for the specified tmux target
            state
                .pane_previews
                .get(session_id)
                .cloned()
                .unwrap_or_else(|| format!("No logs available for target: {session_id}"))
        } else {
            return Err(McpError::resource_not_found(
                format!("Unknown resource URI: {uri}"),
                None,
            ));
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(content, uri.clone())],
        })
    }
}

/// Updates the shared MCP state from the application state.
///
/// This function creates a snapshot of the current application state
/// and updates the shared state used by the MCP server.
///
/// # Arguments
///
/// * `shared_state` - The shared state used by the MCP server.
/// * `app_state` - Reference to the application state.
/// * `pane_preview` - Optional pane preview content for the current session.
/// * `pane_preview` - Optional pane preview content for the current tmux target.
pub fn update_shared_state(
    shared_state: &SharedState,
    app_state: &crate::state::AppState,
    pane_preview: Option<(&str, &str)>, // (tmux_target, content)
) {
    let mut state = shared_state.write().unwrap();

    // Update projects
    state.projects = app_state
        .projects
        .iter()
        .map(ProjectSnapshot::from)
        .collect();

    // Update statuses
    state.statuses = app_state
        .statuses
        .iter()
        .map(|(k, v)| (k.clone(), AgentStatusSnapshot::from(v)))
        .collect();

    // Update pane preview if provided
    if let Some((session_id, content)) = pane_preview {
        state
            .pane_previews
            .insert(session_id.to_string(), content.to_string());
    }
}

/// Runs the MCP server with stdio transport.
///
/// This function should be called when the application is started in MCP server mode.
/// It will block until the client disconnects or an error occurs.
///
/// # Arguments
///
/// * `state` - Shared state that will be read by the server to respond to resource requests.
///
/// # Example
///
/// ```no_run
/// use std::sync::{Arc, RwLock};
/// use vive::mcp::{run_mcp_server, ViveStateSnapshot};
///
/// #[tokio::main]
/// async fn main() {
///     let state = Arc::new(RwLock::new(ViveStateSnapshot::default()));
///     run_mcp_server(state).await.unwrap();
/// }
/// ```
pub async fn run_mcp_server(state: SharedState) -> anyhow::Result<()> {
    let server = ViveMcpServer::new(state);
    let transport = (stdin(), stdout());

    let service = server.serve(transport).await?;

    // Wait for the service to complete
    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{Project, Worktree};

    fn create_test_state() -> ViveStateSnapshot {
        let mut statuses = HashMap::new();
        statuses.insert(
            "user/project-a:main".to_string(),
            AgentStatusSnapshot {
                status: "Working".to_string(),
                icon: "⚙".to_string(),
                status_text: "Working".to_string(),
            },
        );
        statuses.insert(
            "user/project-a:feature-1".to_string(),
            AgentStatusSnapshot {
                status: "Idle".to_string(),
                icon: "•".to_string(),
                status_text: "Idle".to_string(),
            },
        );

        let mut pane_previews = HashMap::new();
        pane_previews.insert(
            "user/project-a:main".to_string(),
            "Some terminal output...".to_string(),
        );

        ViveStateSnapshot {
            projects: vec![ProjectSnapshot {
                name: "user/project-a".to_string(),
                path: "/path/to/user/project-a".to_string(),
                worktrees: vec![
                    WorktreeSnapshot {
                        path: "/path/to/user/project-a".to_string(),
                        commit: "abc123".to_string(),
                        branch: Some("main".to_string()),
                        tmux_target: Some("user/project-a:main".to_string()),
                    },
                    WorktreeSnapshot {
                        path: "/path/to/user/project-a/.worktrees/feature-1".to_string(),
                        commit: "def456".to_string(),
                        branch: Some("feature-1".to_string()),
                        tmux_target: Some("user/project-a:feature-1".to_string()),
                    },
                ],
            }],
            statuses,
            pane_previews,
        }
    }

    #[test]
    fn test_vive_state_snapshot_serialization() {
        let state = create_test_state();
        let json = serde_json::to_string(&state).expect("Should serialize");
        assert!(json.contains("user/project-a"));
        assert!(json.contains("Working"));
    }

    #[test]
    fn test_agent_status_snapshot_from_agent_status() {
        let status = AgentStatus::Working {
            detail: Some("Fixing bug".to_string()),
        };
        let snapshot = AgentStatusSnapshot::from(&status);
        assert_eq!(snapshot.status, "Working");
        assert_eq!(snapshot.icon, "⚙");
        assert!(snapshot.status_text.contains("Fixing bug"));
    }

    #[test]
    fn test_agent_status_snapshot_all_variants() {
        // Test all AgentStatus variants
        let test_cases = vec![
            (AgentStatus::Idle, "Idle"),
            (
                AgentStatus::Working {
                    detail: Some("test".to_string()),
                },
                "Working",
            ),
            (
                AgentStatus::WaitingEdit {
                    path: Some("/path".to_string()),
                },
                "WaitingEdit",
            ),
            (
                AgentStatus::WaitingShell {
                    command: Some("cmd".to_string()),
                },
                "WaitingShell",
            ),
            (AgentStatus::WaitingOther, "WaitingOther"),
            (AgentStatus::Success, "Success"),
            (AgentStatus::Error, "Error"),
        ];

        for (status, expected) in test_cases {
            let snapshot = AgentStatusSnapshot::from(&status);
            assert_eq!(snapshot.status, expected);
            assert!(!snapshot.icon.is_empty());
            assert!(!snapshot.status_text.is_empty());
        }
    }

    #[test]
    fn test_project_snapshot_from_project() {
        let project = Project::new("test-project", "/path/to/project").with_worktrees(vec![
            Worktree::new("/path/to/project", "abc123", Some("main".to_string())),
            Worktree::new(
                "/path/to/project/.worktrees/feature",
                "def456",
                Some("feature".to_string()),
            ),
        ]);

        let snapshot = ProjectSnapshot::from(&project);
        assert_eq!(snapshot.name, "test-project");
        assert_eq!(snapshot.path, "/path/to/project");
        assert_eq!(snapshot.worktrees.len(), 2);
        assert_eq!(snapshot.worktrees[0].branch, Some("main".to_string()));
        assert_eq!(
            snapshot.worktrees[0].tmux_target,
            Some("test-project:main".to_string())
        );
    }

    #[test]
    fn test_worktree_snapshot_detached_head() {
        let worktree = Worktree::new("/path", "abc123", None);
        let snapshot = WorktreeSnapshot::from_worktree(&worktree, "project");
        assert_eq!(snapshot.branch, None);
        assert_eq!(snapshot.tmux_target, None);
    }

    #[test]
    fn test_vive_mcp_server_get_info() {
        let state = Arc::new(RwLock::new(ViveStateSnapshot::default()));
        let server = ViveMcpServer::new(state);
        let info = server.get_info();
        assert_eq!(info.server_info.name, "vive");
        assert!(info.capabilities.resources.is_some());
    }

    #[test]
    fn test_vive_mcp_server_capabilities() {
        let state = Arc::new(RwLock::new(ViveStateSnapshot::default()));
        let server = ViveMcpServer::new(state);
        let info = server.get_info();

        let resources_cap = info.capabilities.resources.expect("Should have resources");
        assert_eq!(resources_cap.subscribe, Some(false));
        assert_eq!(resources_cap.list_changed, Some(false));
    }

    #[test]
    fn test_shared_state_update() {
        let shared_state = Arc::new(RwLock::new(ViveStateSnapshot::default()));

        // Modify the state
        {
            let mut state = shared_state.write().unwrap();
            state.projects.push(ProjectSnapshot {
                name: "new-project".to_string(),
                path: "/path/to/new-project".to_string(),
                worktrees: vec![],
            });
        }

        // Verify the server can read the updated state
        let server = ViveMcpServer::new(shared_state);
        let state = server.get_state();
        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.projects[0].name, "new-project");
    }

    #[test]
    fn test_vive_state_snapshot_default() {
        let state = ViveStateSnapshot::default();
        assert!(state.projects.is_empty());
        assert!(state.statuses.is_empty());
        assert!(state.pane_previews.is_empty());
    }

    #[test]
    fn test_state_concurrent_access() {
        use std::thread;

        let shared_state = Arc::new(RwLock::new(create_test_state()));
        let state_clone = shared_state.clone();

        // Spawn a thread that reads the state
        let handle = thread::spawn(move || {
            let state = state_clone.read().unwrap();
            assert!(!state.projects.is_empty());
        });

        // Main thread also reads
        let state = shared_state.read().unwrap();
        assert!(!state.projects.is_empty());

        handle.join().expect("Thread should complete");
    }
}
