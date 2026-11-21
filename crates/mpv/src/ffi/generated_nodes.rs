//! The test owns the same contiguous node storage that an mpv event would own.
use super::{Node, NodeList, NodeValue, format};
use proptest::{collection, prelude::*};
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_char},
    ptr,
};

#[derive(Default)]
struct NodeStorage {
    strings: Vec<CString>,
    lists: Vec<ListStorage>,
}

struct ListStorage {
    _descriptor: Box<NodeList>,
    _nodes: Box<[Node]>,
    _keys: Option<Box<[*const c_char]>>,
}

impl NodeStorage {
    fn c_string(&mut self, value: &str) -> *const c_char {
        let value = CString::new(value).unwrap();
        let pointer = value.as_ptr();
        // Moving a CString into the storage does not move the owned bytes.
        self.strings.push(value);
        pointer
    }

    fn list(&mut self, children: Vec<Node>, keys: Option<Vec<*const c_char>>) -> Node {
        let children = children.into_boxed_slice();
        let keys = keys.map(Vec::into_boxed_slice);
        let descriptor = Box::new(NodeList {
            count: children.len() as i32,
            values: children.as_ptr(),
            keys: keys.as_ref().map_or(ptr::null(), |keys| keys.as_ptr()),
        });
        let node = Node {
            value: NodeValue { list: &*descriptor },
            format: if keys.is_some() {
                format::NODE_MAP
            } else {
                format::NODE_ARRAY
            },
        };
        self.lists.push(ListStorage {
            _descriptor: descriptor,
            _nodes: children,
            _keys: keys,
        });
        node
    }

    fn encode(&mut self, value: &Value) -> Node {
        let (value, format) = match value {
            Value::Null => (NodeValue { integer: 0 }, format::NONE),
            Value::Bool(flag) => (
                NodeValue {
                    flag: i32::from(*flag),
                },
                format::FLAG,
            ),
            Value::Number(number) => {
                if let Some(integer) = number.as_i64() {
                    (NodeValue { integer }, format::INT64)
                } else {
                    (
                        NodeValue {
                            double: number.as_f64().unwrap(),
                        },
                        format::DOUBLE,
                    )
                }
            }
            Value::String(string) => (
                NodeValue {
                    string: self.c_string(string),
                },
                format::STRING,
            ),
            Value::Array(entries) => {
                let nodes = entries.iter().map(|value| self.encode(value)).collect();
                return self.list(nodes, None);
            }
            Value::Object(entries) => {
                let keys = entries.keys().map(|key| self.c_string(key)).collect();
                let nodes = entries.values().map(|value| self.encode(value)).collect();
                return self.list(nodes, Some(keys));
            }
        };
        Node { value, format }
    }
}

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
        let node = storage.encode(&expected);
        // SAFETY: encode tags each node's active union arm and ends with the
        // owning descriptor/child/string allocations still held by storage.
        let copy = unsafe { node.copy_json() }.unwrap();
        drop(storage);
        prop_assert_eq!(copy, expected);
    }
}
