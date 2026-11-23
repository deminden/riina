//! Round-trip generated values through the native input and output representation.
use super::node_input::NodeStorage;
use proptest::{collection, prelude::*};
use serde_json::{Value, json};

fn node_values() -> impl Strategy<Value = Value> {
    let scalar = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|value| json!(value)),
        any::<i32>().prop_map(|value| json!(f64::from(value) / 8.0)),
        "[a-zA-Z0-9 _-]{0,36}".prop_map(Value::String),
    ];
    scalar.prop_recursive(5, 64, 8, |value| {
        prop_oneof![
            collection::vec(value.clone(), 0..8).prop_map(Value::Array),
            collection::btree_map("[a-z]{0,12}", value, 0..8)
                .prop_map(|values| Value::Object(values.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn copies_generated_node_trees_while_their_storage_is_live(expected in node_values()) {
        let mut storage = NodeStorage::default();
        let node = storage.encode(&expected, 0).unwrap();
        // SAFETY: encode tags each node's active union arm and ends with the
        // owning descriptor/child/string allocations still held by storage.
        let copy = unsafe { node.copy_json() }.unwrap();
        drop(storage);
        prop_assert_eq!(copy, expected);
    }
}
