use std::collections::BTreeMap;
use std::sync::Arc;

use crate::definition::{DynTool, Tool, ToolDefinition};
use crate::environment::{
    EnvironmentGetTool, EnvironmentListTool, EnvironmentSetTool, EnvironmentUnsetTool,
};
use crate::filesystem::{
    FileSystemCreateTool, FileSystemDeleteTool, FileSystemEditTool, FileSystemListTool,
    FileSystemMkdirTool, FileSystemReadTool, FileSystemWriteTool,
};
use crate::shell::ShellExecuteTool;
use crate::system::{
    SystemArchitectureTool, SystemHostnameTool, SystemInfoTool, SystemNetworkConnectionsTool,
    SystemNetworkInterfacesTool, SystemNetworkPortCheckTool, SystemNetworkPortProcessTool,
    SystemPlatformTool, SystemProcessFindTool, SystemProcessInspectTool, SystemProcessListTool,
    SystemProcessTerminateTool, SystemRuntimeVersionTool, SystemRuntimeWhichTool, SystemUptimeTool,
};
use crate::workspace_tools::{WorkspaceDetectTool, WorkspaceInspectTool};

/// Thread-safe registry maintaining all registered tools available to Hades and models.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, DynTool>,
}

impl ToolRegistry {
    /// Creates an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: BTreeMap::new(),
        }
    }

    /// Registers a tool instance under its definition name.
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let def = tool.definition();
        self.tools.insert(def.name, Arc::new(tool));
    }

    /// Registers an Arc-wrapped tool instance.
    pub fn register_arc(&mut self, tool: DynTool) {
        let def = tool.definition();
        self.tools.insert(def.name, tool);
    }

    /// Retrieves a tool by its name.
    pub fn get(&self, name: &str) -> Option<DynTool> {
        self.tools.get(name).cloned()
    }

    /// Lists definitions of all registered tools.
    pub fn list(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Checks if a tool with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Returns the total number of registered tools.
    pub fn count(&self) -> usize {
        self.tools.len()
    }

    /// Builds the standard default registry populated with all built-in Hades Phase 3 tools.
    pub fn default_registry() -> Self {
        let mut reg = Self::new();

        // Filesystem tools
        reg.register(FileSystemListTool);
        reg.register(FileSystemReadTool);
        reg.register(FileSystemCreateTool);
        reg.register(FileSystemWriteTool);
        reg.register(FileSystemEditTool);
        reg.register(FileSystemDeleteTool);
        reg.register(FileSystemMkdirTool);

        // Shell & process tools
        reg.register(ShellExecuteTool);

        // Environment tools
        reg.register(EnvironmentGetTool);
        reg.register(EnvironmentListTool);
        reg.register(EnvironmentSetTool);
        reg.register(EnvironmentUnsetTool);

        // Workspace tools
        reg.register(WorkspaceInspectTool);
        reg.register(WorkspaceDetectTool);

        // System Diagnostic & Inspection tools
        reg.register(SystemInfoTool);
        reg.register(SystemPlatformTool);
        reg.register(SystemArchitectureTool);
        reg.register(SystemHostnameTool);
        reg.register(SystemUptimeTool);
        reg.register(SystemProcessListTool);
        reg.register(SystemProcessInspectTool);
        reg.register(SystemProcessFindTool);
        reg.register(SystemProcessTerminateTool);
        reg.register(SystemNetworkInterfacesTool);
        reg.register(SystemNetworkPortCheckTool);
        reg.register(SystemNetworkPortProcessTool);
        reg.register(SystemNetworkConnectionsTool);
        reg.register(SystemRuntimeWhichTool);
        reg.register(SystemRuntimeVersionTool);

        reg
    }
}
