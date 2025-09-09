//! Test to verify that our custom serde implementation works without JSON
//! This test demonstrates direct RichStruct ↔ Entity conversion

use sentio_sdk::entity::{ToRichValue, FromRichValue, to_rich_struct, from_rich_struct};
use sentio_sdk::entity::types::Timestamp;
use sentio_sdk::common::rich_value;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TestEntity {
    pub id: String,
    pub name: String,
    pub value: i32,
    pub score: f64,
    pub active: bool,
    pub tags: Vec<String>,
    pub optional_field: Option<i32>,
}

#[test]
fn test_complete_direct_serialization_no_json() {
    // Create a test entity
    let entity = TestEntity {
        id: "test-123".to_string(),
        name: "Test Entity".to_string(),
        value: 42,
        score: 3.14,
        active: true,
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        optional_field: Some(100),
    };

    println!("🚀 Testing DIRECT serialization to RichStruct (NO JSON!)");

    // Test DIRECT serialization to RichStruct - NO JSON BRIDGE!
    let rich_struct = to_rich_struct(&entity).expect("Direct serialization should work");
    
    // Verify that the RichStruct contains the expected fields
    assert!(rich_struct.fields.contains_key("id"));
    assert!(rich_struct.fields.contains_key("name"));
    assert!(rich_struct.fields.contains_key("value"));
    assert!(rich_struct.fields.contains_key("score"));
    assert!(rich_struct.fields.contains_key("active"));
    assert!(rich_struct.fields.contains_key("tags"));
    assert!(rich_struct.fields.contains_key("optional_field"));

    println!("✅ Direct serialization successful - struct has {} fields", rich_struct.fields.len());

    // Test DIRECT deserialization back to entity - NO JSON BRIDGE!
    let converted_entity: TestEntity = from_rich_struct(&rich_struct)
        .expect("Direct deserialization should work");

    // Verify round-trip conversion works perfectly
    assert_eq!(entity, converted_entity);

    println!("✅ Direct deserialization successful - round-trip conversion perfect!");
    println!("🎉 SUCCESS: Complete direct RichStruct ↔ Entity conversion without JSON!");
}

#[test]
fn test_direct_scalar_types() {
    println!("🧪 Testing direct scalar type conversions");

    // Test String
    let s = "hello world".to_string();
    let rich_value = s.to_rich_value().unwrap();
    let converted_s = String::from_rich_value(&rich_value).unwrap();
    assert_eq!(s, converted_s);
    println!("✅ String conversion works");

    // Test i32
    let i = 42i32;
    let rich_value = i.to_rich_value().unwrap();
    let converted_i = i32::from_rich_value(&rich_value).unwrap();
    assert_eq!(i, converted_i);
    println!("✅ i32 conversion works");

    // Test f64
    let f = 3.14159f64;
    let rich_value = f.to_rich_value().unwrap();
    let converted_f = f64::from_rich_value(&rich_value).unwrap();
    assert_eq!(f, converted_f);
    println!("✅ f64 conversion works");

    // Test bool
    let b = true;
    let rich_value = b.to_rich_value().unwrap();
    let converted_b = bool::from_rich_value(&rich_value).unwrap();
    assert_eq!(b, converted_b);
    println!("✅ bool conversion works");

    // Test Vec<String>
    let vec = vec!["item1".to_string(), "item2".to_string(), "item3".to_string()];
    let rich_value = vec.to_rich_value().unwrap();
    let converted_vec = Vec::<String>::from_rich_value(&rich_value).unwrap();
    assert_eq!(vec, converted_vec);
    println!("✅ Vec<String> conversion works");

    // Test Option<i32>
    let some_value = Some(123i32);
    let rich_value = some_value.to_rich_value().unwrap();
    let converted_some = Option::<i32>::from_rich_value(&rich_value).unwrap();
    assert_eq!(some_value, converted_some);
    println!("✅ Option<i32> (Some) conversion works");

    let none_value: Option<i32> = None;
    let rich_value = none_value.to_rich_value().unwrap();
    let converted_none = Option::<i32>::from_rich_value(&rich_value).unwrap();
    assert_eq!(none_value, converted_none);
    println!("✅ Option<i32> (None) conversion works");

    println!("🎉 All scalar type conversions work perfectly!");
}

#[test] 
fn test_primitive_to_struct_wrapping() {
    println!("🔧 Testing primitive value wrapping in structs");

    // Test that primitives get wrapped in structs with "value" field
    let number = 42i32;
    let rich_struct = to_rich_struct(&number).unwrap();
    
    // Should have exactly one field called "value"
    assert_eq!(rich_struct.fields.len(), 1);
    assert!(rich_struct.fields.contains_key("value"));
    
    // Test round-trip conversion
    let converted_number: i32 = from_rich_struct(&rich_struct).unwrap();
    assert_eq!(number, converted_number);
    
    println!("✅ Primitive wrapping and unwrapping works correctly");
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct NestedEntity {
    pub outer_field: String,
    pub inner: InnerEntity,
    pub list_of_inner: Vec<InnerEntity>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct InnerEntity {
    pub inner_id: i32,
    pub inner_name: String,
}

#[test]
fn test_nested_struct_serialization() {
    println!("🏗️  Testing nested struct serialization (NO JSON!)");

    let nested = NestedEntity {
        outer_field: "outer".to_string(),
        inner: InnerEntity {
            inner_id: 1,
            inner_name: "inner1".to_string(),
        },
        list_of_inner: vec![
            InnerEntity {
                inner_id: 2,
                inner_name: "inner2".to_string(),
            },
            InnerEntity {
                inner_id: 3,
                inner_name: "inner3".to_string(),
            },
        ],
    };

    // Direct serialization of nested structure
    let rich_struct = to_rich_struct(&nested).unwrap();
    
    // Should have all the outer fields
    assert!(rich_struct.fields.contains_key("outer_field"));
    assert!(rich_struct.fields.contains_key("inner"));
    assert!(rich_struct.fields.contains_key("list_of_inner"));
    
    // Test round-trip
    let converted_nested: NestedEntity = from_rich_struct(&rich_struct).unwrap();
    assert_eq!(nested, converted_nested);
    
    println!("✅ Nested struct serialization works perfectly without JSON!");
}

#[test]
fn test_timestamp_serialization_fix() {
    println!("⏰ Testing custom Timestamp serialization (should use timestamp_value, NOT string_value!)");

    // Create a specific timestamp to test with (similar to the production issue)
    let timestamp = Timestamp::new(
        DateTime::parse_from_rfc3339("1970-01-10T18:35:30Z").unwrap().with_timezone(&Utc)
    );

    println!("🔍 Testing timestamp: {:?}", timestamp);

    // Test direct RichValue serialization
    let rich_value = timestamp.to_rich_value().expect("Timestamp should serialize to RichValue");
    
    // CRITICAL TEST: Verify it's using timestamp_value, NOT string_value
    match &rich_value.value {
        Some(rich_value::Value::TimestampValue(ts)) => {
            println!("✅ SUCCESS: Timestamp correctly serialized as timestamp_value!");
            println!("   - Timestamp seconds: {}", ts.seconds);
            println!("   - Timestamp nanos: {}", ts.nanos);
            
            // Verify the timestamp values are correct
            assert_eq!(ts.seconds, timestamp.timestamp());
            assert_eq!(ts.nanos, timestamp.timestamp_subsec_nanos() as i32);
        }
        Some(rich_value::Value::StringValue(s)) => {
            panic!("❌ FAILURE: Timestamp incorrectly serialized as string_value: '{}'", s);
        }
        other => {
            panic!("❌ FAILURE: Timestamp serialized as unexpected type: {:?}", other);
        }
    }

    // Test round-trip conversion
    let converted_timestamp = Timestamp::from_rich_value(&rich_value)
        .expect("Should be able to deserialize timestamp");
    assert_eq!(timestamp, converted_timestamp);
    println!("✅ Round-trip conversion successful!");

    // Test within a struct (this simulates the real-world entity scenario)
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct EntityWithTimestamp {
        pub id: String,
        pub timestamp: Timestamp,
    }

    let entity = EntityWithTimestamp {
        id: "test-entity".to_string(),
        timestamp: timestamp.clone(),
    };

    println!("🏗️  Testing Timestamp within entity struct...");

    // Serialize the entity to RichStruct
    let rich_struct = to_rich_struct(&entity).expect("Entity serialization should work");
    
    // Verify the timestamp field exists
    assert!(rich_struct.fields.contains_key("timestamp"));
    
    // Get the timestamp field and verify it's a timestamp_value
    let timestamp_field = rich_struct.fields.get("timestamp").unwrap();
    match &timestamp_field.value {
        Some(rich_value::Value::TimestampValue(ts)) => {
            println!("✅ SUCCESS: Entity timestamp field correctly serialized as timestamp_value!");
            println!("   - Timestamp seconds: {}", ts.seconds);
            println!("   - Timestamp nanos: {}", ts.nanos);
        }
        Some(rich_value::Value::StringValue(s)) => {
            panic!("❌ FAILURE: Entity timestamp field incorrectly serialized as string_value: '{}'", s);
        }
        other => {
            panic!("❌ FAILURE: Entity timestamp field serialized as unexpected type: {:?}", other);
        }
    }

    // Test entity round-trip conversion
    let converted_entity: EntityWithTimestamp = from_rich_struct(&rich_struct)
        .expect("Entity deserialization should work");
    assert_eq!(entity, converted_entity);
}