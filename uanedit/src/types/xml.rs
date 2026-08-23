use indexmap::IndexMap;
use serde::{
    Deserialize,
    Serialize,
};

/// XML kept exactly as written, so content this crate does not model survives a round trip.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct XmlElement {
    /// The tag as written, prefix included.
    pub name: String,
    pub attributes: IndexMap<String, String>,
    pub children: Vec<XmlNode>,
}

impl XmlElement {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attributes: IndexMap::new(),
            children: Vec::new(),
        }
    }

    pub fn local_name(&self) -> &str {
        match self.name.split_once(':') {
            Some((_, local)) => local,
            None => &self.name,
        }
    }

    pub fn attribute(
        &self,
        name: &str,
    ) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    pub fn elements(&self) -> impl Iterator<Item = &XmlElement> {
        self.children.iter().filter_map(|child| match child {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
    }

    pub fn child(
        &self,
        local_name: &str,
    ) -> Option<&XmlElement> {
        self.elements()
            .find(|element| element.local_name() == local_name)
    }

    /// The text of every direct text child, concatenated.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|child| match child {
                XmlNode::Text(text) | XmlNode::CData(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn push(
        &mut self,
        child: impl Into<XmlNode>,
    ) {
        self.children.push(child.into());
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
    CData(String),
    Comment(String),
    ProcessingInstruction(String),
}

impl From<XmlElement> for XmlNode {
    fn from(element: XmlElement) -> Self {
        Self::Element(element)
    }
}
