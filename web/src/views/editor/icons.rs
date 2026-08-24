//! Material Symbols analogies for the eight node classes (ui/material.md §5.3).
//!
//! Material has no OPC UA vocabulary, so these are chosen for distinct silhouettes at 18 px and for
//! pairing an instance class with the class that types it:
//!
//! | Class         | Symbol            | Reading                                              |
//! | ------------- | ----------------- | ---------------------------------------------------- |
//! | Object        | `deployed_code`   | a solid box — a thing that exists in the address space |
//! | ObjectType    | `category`        | the classifier those things are sorted into           |
//! | Variable      | `label`           | a tag: a named value hung on something                |
//! | VariableType  | `sell`            | the same tag drawn hollow — the template of a value   |
//! | Method        | `function`        | `f(x)`: the one node class you call                   |
//! | View          | `visibility`      | a chosen way of looking at the graph                  |
//! | DataType      | `data_object`     | `{ }`: the shape a value takes                        |
//! | ReferenceType | `arrow_right_alt` | the arrow itself                                      |

use uanedit::nodes::NodeClass;

pub fn class_icon(node_class: NodeClass) -> &'static str {
    match node_class {
        NodeClass::Object => "deployed_code",
        NodeClass::ObjectType => "category",
        NodeClass::Variable => "label",
        NodeClass::VariableType => "sell",
        NodeClass::Method => "function",
        NodeClass::View => "visibility",
        NodeClass::DataType => "data_object",
        NodeClass::ReferenceType => "arrow_right_alt",
        NodeClass::Unspecified => "help",
    }
}
