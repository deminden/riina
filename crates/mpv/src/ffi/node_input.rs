//! Own all pointers in the node until libmpv copies the property value.
use super::{Node, NodeList, NodeValue, format};
use crate::EngineError;
use serde_json::Value;
use std::{
    ffi::{CString, c_char, c_int},
    ptr,
};

#[derive(Default)]
pub(in crate::ffi) struct NodeStorage {
    strings: Vec<CString>,
    lists: Vec<ListStorage>,
}

struct ListStorage {
    _descriptor: Box<NodeList>,
    _nodes: Box<[Node]>,
    _keys: Option<Box<[*const c_char]>>,
}

pub(crate) fn property_string(value: &str) -> Result<CString, EngineError> {
    CString::new(value).map_err(|_| EngineError::InvalidArgument {
        reason: "Native property strings cannot contain NUL",
    })
}

impl NodeStorage {
    fn c_string(&mut self, value: &str) -> Result<*const c_char, EngineError> {
        let value = property_string(value)?;
        let pointer = value.as_ptr();
        // Moving a CString into the arena does not move its bytes.
        self.strings.push(value);
        Ok(pointer)
    }

    fn list(
        &mut self,
        children: Vec<Node>,
        keys: Option<Vec<*const c_char>>,
    ) -> Result<Node, EngineError> {
        let children = children.into_boxed_slice();
        let keys = keys.map(Vec::into_boxed_slice);
        let descriptor = Box::new(NodeList {
            count: c_int::try_from(children.len()).map_err(|_| EngineError::InvalidArgument {
                reason: "Native property list has too many entries",
            })?,
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
        Ok(node)
    }

    pub(in crate::ffi) fn encode(
        &mut self,
        value: &Value,
        depth: usize,
    ) -> Result<Node, EngineError> {
        if depth > 32 {
            return Err(EngineError::InvalidArgument {
                reason: "Native property nesting exceeds 32 levels",
            });
        }
        let (value, format) = match value {
            Value::Null => (NodeValue { integer: 0 }, format::NONE),
            Value::Bool(flag) => (
                NodeValue {
                    flag: c_int::from(*flag),
                },
                format::FLAG,
            ),
            Value::Number(number) => {
                if let Some(integer) = number.as_i64() {
                    (NodeValue { integer }, format::INT64)
                } else {
                    let double = number.as_f64().ok_or(EngineError::InvalidArgument {
                        reason: "Native property number cannot be represented",
                    })?;
                    (NodeValue { double }, format::DOUBLE)
                }
            }
            Value::String(string) => (
                NodeValue {
                    string: self.c_string(string)?,
                },
                format::STRING,
            ),
            Value::Array(entries) => {
                let nodes = entries
                    .iter()
                    .map(|value| self.encode(value, depth + 1))
                    .collect::<Result<_, _>>()?;
                return self.list(nodes, None);
            }
            Value::Object(entries) => {
                let keys = entries
                    .keys()
                    .map(|key| self.c_string(key))
                    .collect::<Result<_, _>>()?;
                let nodes = entries
                    .values()
                    .map(|value| self.encode(value, depth + 1))
                    .collect::<Result<_, _>>()?;
                return self.list(nodes, Some(keys));
            }
        };
        Ok(Node { value, format })
    }
}

pub(crate) fn with_node<T>(
    value: &Value,
    apply: impl FnOnce(&mut Node) -> Result<T, EngineError>,
) -> Result<T, EngineError> {
    let mut storage = NodeStorage::default();
    let mut node = storage.encode(value, 0)?;
    apply(&mut node)
}
