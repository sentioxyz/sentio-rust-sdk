use crate::core::plugin::FullPlugin;
use crate::core::{RuntimeContext, RUNTIME_CONTEXT};
use crate::{DataBinding, ProcessResult};
use dashmap::DashMap;

pub struct PluginManager {
    pub(crate) plugins: DashMap<String, Box<dyn FullPlugin>>,
    pub(crate) handler_type_owner: DashMap<crate::processor::HandlerType, String>,
}

impl PluginManager {
    /// Get or create a plugin by type. If plugin doesn't exist, create it using Default::default().
    /// This method performs the registration but returns unit since DashMap references are complex.
    /// Use get_plugin_typed to retrieve the plugin afterwards.
    pub fn ensure_plugin<P>(&self)
    where
        P: FullPlugin + Default + 'static,
    {
        // Create a temporary instance to get the plugin name
        let name = P::name();

        // Check if plugin already exists, if not insert new one
        if !self.plugins.contains_key(name) {
            let temp_plugin = P::default();
            for handler_type in temp_plugin.handler_types() {
                if self.handler_type_owner.contains_key(handler_type) {
                    if let Some(existing_owner) = self.handler_type_owner.get(handler_type) {
                        panic!(
                            "Handler type {:?} already owned by plugin {}",
                            handler_type, existing_owner.value()
                        );
                    }
                }
                self.handler_type_owner
                    .insert(*handler_type, name.to_string());
            }
            self.plugins.insert(name.to_string(), Box::new(temp_plugin));
        }
    }

    /// Get or create a plugin by type and execute a closure with mutable access to it
    pub fn with_plugin_mut<P, F, R>(&self, f: F) -> R
    where
        P: FullPlugin + Default + 'static,
        F: FnOnce(&mut P) -> R,
    {
        self.ensure_plugin::<P>();
        let name = P::name();
        let mut plugin_ref = self.plugins.get_mut(name).expect("Plugin should exist after ensure");
        let any_plugin = plugin_ref.value_mut().as_mut() as &mut dyn std::any::Any;
        let typed_plugin = any_plugin
            .downcast_mut::<P>()
            .expect("Plugin type mismatch");
        f(typed_plugin)
    }

    /// Get the total number of processors across all plugins
    pub fn total_processor_count(&self) -> usize {
        self.plugins
            .iter()
            .map(|entry| entry.value().processor_count())
            .sum()
    }

    /// Get all chain IDs from all processors across all plugins
    pub fn get_all_chain_ids(&self) -> Vec<String> {
        let mut chain_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Collect chain IDs from all plugins
        for entry in self.plugins.iter() {
            entry.value().chain_ids().into_iter().for_each(|chain_id| {
                chain_ids.insert(chain_id);
            })
        }

        chain_ids.into_iter().collect()
    }

    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Check if plugin can handle a specific handler type
    pub fn plugin_can_handle(&self, name: &str, handler_type: crate::processor::HandlerType) -> bool {
        self.plugins.get(name)
            .map(|entry| entry.value().can_handle_type(handler_type))
            .unwrap_or(false)
    }

    /// Get a plugin by name with concrete type (read-only access)
    pub fn with_plugin<P, F, R>(&self, f: F) -> Option<R>
    where
        P: FullPlugin + 'static,
        F: FnOnce(&P) -> R,
    {
        let name = P::name();
        self.plugins.get(name).and_then(|plugin_ref| {
            let any_plugin = plugin_ref.value().as_ref() as &dyn std::any::Any;
            let typed_plugin = any_plugin.downcast_ref::<P>()?;
            Some(f(typed_plugin))
        })
    }

    /// Configure all plugins for a specific chain_id
    pub fn configure_all_plugins(
        &self,
        response: &mut crate::processor::ConfigureHandlersResponse,
    ) {
        // First collect the plugin names to avoid borrow checker issues
        let plugin_names: Vec<String> = self.plugins.iter().map(|entry| entry.key().clone()).collect();
        
        for plugin_name in plugin_names {
            if let Some(mut plugin_entry) = self.plugins.get_mut(&plugin_name) {
                tracing::debug!("Configuring plugin: {}", plugin_name);
                plugin_entry.value_mut().configure(response);
                tracing::debug!(
                    "Plugin '{}' contributed {} contract configs",
                    plugin_name,
                    response.contract_configs.len()
                );
            }
        }
    }

    /// Get names of all plugins that can handle a specific handler type
    pub fn get_plugin_names_for_handler_type(
        &self,
        handler_type: crate::processor::HandlerType,
    ) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|entry| entry.value().can_handle_type(handler_type))
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub async fn process(
        &self,
        data: &DataBinding,
        runtime_context: RuntimeContext,
    ) -> anyhow::Result<ProcessResult> {
        let handler_type = crate::processor::HandlerType::try_from(data.handler_type)?;
        let plugin_name = self.handler_type_owner.get(&handler_type)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                anyhow::anyhow!("No plugin registered for handler type: {:?}", handler_type)
            })?;

        let plugin = self
            .plugins
            .get(&plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", plugin_name))?;

        RUNTIME_CONTEXT
            .scope(runtime_context, plugin.value().process_binding(data))
            .await
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self {
            plugins: DashMap::new(),
            handler_type_owner: DashMap::new(),
        }
    }
}
