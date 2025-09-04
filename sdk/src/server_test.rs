#[cfg(test)]
mod tests {
    use crate::Server;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_concurrent_plugin_manager_access() {
        let server = Server::new();
        
        // Test concurrent read access to plugin manager
        let pm1 = server.plugin_manager.clone();
        let pm2 = server.plugin_manager.clone();
        
        let handle1 = tokio::spawn(async move {
            let _guard = pm1.read().await;
            sleep(Duration::from_millis(10)).await;
            "task1_done"
        });
        
        let handle2 = tokio::spawn(async move {
            let _guard = pm2.read().await;
            sleep(Duration::from_millis(10)).await;
            "task2_done"
        });
        
        let (result1, result2) = tokio::join!(handle1, handle2);
        assert_eq!(result1.unwrap(), "task1_done");
        assert_eq!(result2.unwrap(), "task2_done");
    }

    #[tokio::test]
    async fn test_write_access_exclusivity() {
        let server = Server::new();
        let pm = server.plugin_manager.clone();
        
        // Test that write access is exclusive
        let start = std::time::Instant::now();
        
        let handle1 = tokio::spawn({
            let pm = pm.clone();
            async move {
                let _guard = pm.write().await;
                sleep(Duration::from_millis(50)).await;
                std::time::Instant::now()
            }
        });
        
        let handle2 = tokio::spawn({
            let pm = pm.clone();
            async move {
                sleep(Duration::from_millis(10)).await; // Start slightly after handle1
                let _guard = pm.write().await;
                std::time::Instant::now()
            }
        });
        
        let (time1, time2) = tokio::join!(handle1, handle2);
        let time1 = time1.unwrap();
        let time2 = time2.unwrap();
        
        // Second write should happen after first one completes
        assert!(time2 > time1);
        assert!(time2.duration_since(start) >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_databinding_zero_copy_optimization() {
        todo!();
        /*// This test demonstrates that we're not unnecessarily copying DataBinding
        // by using the binding directly from the stream
        let server = Server::new();
        
        // Create a mock DataBinding (this would normally come from the protobuf stream)
        let binding = crate::DataBinding {
            data: None,
            handler_type: 0, // Mock handler type
            handler_ids: vec![1, 2, 3],
            chain_id: "test-chain".to_string(),
        };
        
        // Test that we can process the binding directly without copying
        let pm = server.plugin_manager.read().await;
        
        // Create a mock RuntimeContext for testing
        let (tx, _rx) = tokio::sync::mpsc::channel(1);

        // let runtime_context = crate::core::RuntimeContext::new_with_empty_metadata(tx, 1, );
        //
        // // This should work without any intermediate DataBinding creation
        // // The process method takes &DataBinding, so no copy is made
        // let result = pm.process(&binding, runtime_context).await;
        //
        // // We expect this to fail since no plugins are registered, but the important
        // // thing is that it compiles and doesn't require copying the binding
        // assert!(result.is_err());
        // assert!(result.unwrap_err().to_string().contains("No plugin registered"));*/
    }
}