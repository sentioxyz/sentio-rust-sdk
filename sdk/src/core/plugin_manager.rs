use std::collections::HashMap;
use crate::{BaseProcessor, Plugin};

#[derive(Default)]
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
}

impl PluginManager {
    /// Get or create a plugin by type. If plugin doesn't exist, create it using Default::default().
    pub fn plugin<P: Plugin + Default + 'static>(&mut self) -> &mut P {
        // Create a temporary instance to get the plugin name
        let name = P::name();

        // Check if plugin already exists, if not insert new one
        if !self.plugins.contains_key(name) {
            let temp_plugin = P::default();
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
    
    /// Iterate over all processors across all plugins
    pub fn iter_processors(&self) -> impl Iterator<Item = &Box<dyn BaseProcessor>> {
        self.plugins.values().flat_map(|plugin| plugin.iter_processors())
    }
    
    /// Get all chain IDs from all processors across all plugins
    pub fn get_all_chain_ids(&self) -> Vec<String> {
        let mut chain_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        // Collect chain IDs from all plugins
        for processor in self.iter_processors() {
            chain_ids.insert(processor.chain_id().to_string());
        }
        
        chain_ids.into_iter().collect()
    }
    
    /// Get a plugin by name
    pub fn get_plugin(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }
    
    /// Get a plugin by name with concrete type
    pub fn get_plugin_typed<P: Plugin + 'static>(&self, name: &str) -> Option<&P> {
        self.plugins.get(name).and_then(|plugin| {
            let any_plugin = plugin.as_ref() as &dyn std::any::Any;
            any_plugin.downcast_ref::<P>()
        })
    }
}


