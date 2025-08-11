use std::collections::HashMap;
use crate::{ConfigureHandlersRequest, DataBinding, ProcessResult};
use crate::core::plugin::FullPlugin;

#[derive(Default)]
pub struct PluginManager {
    pub(crate) plugins: HashMap<String, Box<dyn FullPlugin>>,
    pub(crate) handler_type_owner: HashMap<crate::processor::HandlerType, String>,
}

impl PluginManager {
    /// Get or create a plugin by type. If plugin doesn't exist, create it using Default::default().
    pub fn plugin<P>(&mut self) -> &mut P 
    where 
        P: FullPlugin + Default + 'static
    {
        // Create a temporary instance to get the plugin name
        let name = P::name();

        // Check if plugin already exists, if not insert new one
        if !self.plugins.contains_key(name) {
            let temp_plugin = P::default();
            for handler_type in temp_plugin.handler_types() {
                if self.handler_type_owner.contains_key(handler_type) {
                    panic!("Handler type {:?} already owned by plugin {}", handler_type, self.handler_type_owner[handler_type]);
                }
                self.handler_type_owner.insert(*handler_type, name.to_string());
            }
            self.plugins.insert(name.to_string(), Box::new(temp_plugin));
        }

        // Get the plugin and downcast it
        let plugin_box = self.plugins.get_mut(name).unwrap();
        let any_plugin = plugin_box.as_mut() as &mut dyn std::any::Any;
        any_plugin.downcast_mut::<P>().expect("Plugin type mismatch")
    }
    
    /// Get the total number of processors across all plugins
    pub fn total_processor_count(&self) -> usize {
        self.plugins.values().map(|plugin| plugin.processor_count()).sum()
    }

    
    /// Get all chain IDs from all processors across all plugins
    pub fn get_all_chain_ids(&self) -> Vec<String> {
        let mut chain_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        // Collect chain IDs from all plugins
        for plugin in self.plugins.values() {
            plugin.chain_ids().into_iter().for_each(|chain_id| {
                chain_ids.insert(chain_id);
            })
        }
        
        chain_ids.into_iter().collect()
    }
    
    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn FullPlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }
    
    /// Get a plugin by name with concrete type
    pub fn get_plugin_typed<P: FullPlugin + 'static>(&self, name: &str) -> Option<&P> {
        self.plugins.get(name).and_then(|plugin| {
            let any_plugin = plugin.as_ref() as &dyn std::any::Any;
            any_plugin.downcast_ref::<P>()
        })
    }

    /// Configure all plugins for a specific chain_id
    pub fn configure_all_plugins(&mut self, request: ConfigureHandlersRequest, response: &mut crate::processor::ConfigureHandlersResponse) {
        for (plugin_name, plugin) in self.plugins.iter_mut() {
            tracing::debug!("Configuring plugin: {}", plugin_name);
            plugin.configure(&request, response);
            tracing::debug!("Plugin '{}' contributed {} contract configs", 
                           plugin_name, response.contract_configs.len());
        }
    }
    
    /// Get all plugins that can handle a specific handler type
    pub fn get_plugins_for_handler_type(&self, handler_type: crate::processor::HandlerType) -> Vec<(&String, &dyn FullPlugin)> {
        self.plugins.iter()
            .filter(|(_, plugin)| plugin.can_handle_type(handler_type))
            .map(|(name, plugin)| (name, plugin.as_ref()))
            .collect()
    }
    
    pub async fn process(&self, data: &DataBinding) -> anyhow::Result<ProcessResult> {
        let handler_type = crate::processor::HandlerType::try_from(data.handler_type)?;
        let plugin_name = self.handler_type_owner.get(&handler_type)
            .ok_or_else(|| anyhow::anyhow!("No plugin registered for handler type: {:?}", handler_type))?;
        
        let plugin = self.plugins.get(plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin not found: {}", plugin_name))?;
        
        plugin.process_binding(data).await
    }
}


