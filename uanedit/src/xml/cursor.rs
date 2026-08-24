use indexmap::IndexMap;
use quick_xml::XmlVersion;
use quick_xml::events::{
    BytesStart,
    Event,
};
use quick_xml::reader::Reader;

use crate::error::DocumentError;
use crate::types::xml::{
    XmlElement,
    XmlNode,
};
use crate::xml::span::Span;

/// A pull parser that reports where every event sat in the source.
///
/// Spans are absolute offsets into the original text, including a byte-order mark that quick-xml
/// consumes without counting.
pub(crate) struct Cursor<'a> {
    source: &'a str,
    reader: Reader<&'a [u8]>,
    origin: usize,
    position: usize,
    end: Span,
}

/// An element start, whether or not it was written self-closing.
pub(crate) struct Tag<'a> {
    pub start: BytesStart<'a>,
    pub span: Span,
    pub empty: bool,
}

impl<'a> Tag<'a> {
    pub fn name(&self) -> &str {
        self.start.name().into_inner()
    }

    pub fn local_name(&self) -> &str {
        match self.name().split_once(':') {
            Some((_, local)) => local,
            None => self.name(),
        }
    }

    /// The attributes as written, in order. An attribute the parser rejects — a repeated one, or
    /// a value naming an entity nothing declares — is passed over rather than taking the rest of
    /// the element with it.
    pub fn attributes(&self) -> IndexMap<String, String> {
        let mut attributes = IndexMap::new();
        for attribute in self.start.attributes() {
            let Ok(attribute) = attribute else { continue };
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .unwrap_or(attribute.value.clone());
            attributes.insert(attribute.key.as_ref().to_owned(), value.into_owned());
        }
        attributes
    }
}

impl<'a> Cursor<'a> {
    pub fn new(source: &'a str) -> Self {
        let origin = if source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        Self {
            source,
            reader: Reader::from_str(source),
            origin,
            position: origin,
            end: Span::default(),
        }
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    /// The span of the end tag that closed the element last read.
    pub fn end(&self) -> Span {
        self.end
    }

    pub fn read(&mut self) -> Result<(Event<'a>, Span), DocumentError> {
        let start = self.position;
        let event = self.reader.read_event().map_err(|error| {
            let offset = usize::try_from(self.reader.error_position()).unwrap_or(usize::MAX);
            DocumentError::Syntax {
                offset: offset.saturating_add(self.origin),
                message: error.to_string(),
            }
        })?;
        self.position = usize::try_from(self.reader.buffer_position())
            .unwrap_or(usize::MAX)
            .saturating_add(self.origin);
        Ok((event, Span::new(start, self.position)))
    }

    /// The next child element of the element being read, or `None` at its end tag.
    pub fn tag(&mut self) -> Result<Option<Tag<'a>>, DocumentError> {
        loop {
            let (event, span) = self.read()?;
            match event {
                Event::Start(start) => {
                    return Ok(Some(Tag {
                        start,
                        span,
                        empty: false,
                    }));
                }
                Event::Empty(start) => {
                    return Ok(Some(Tag {
                        start,
                        span,
                        empty: true,
                    }));
                }
                Event::End(_) => {
                    self.end = span;
                    return Ok(None);
                }
                Event::Eof => {
                    self.end = Span::new(span.start, span.start);
                    return Ok(None);
                }
                _ => {}
            }
        }
    }

    /// The text content of an element, with entity references resolved.
    pub fn text(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<String, DocumentError> {
        if tag.empty {
            self.end = Span::new(tag.span.end, tag.span.end);
            return Ok(String::new());
        }
        let mut text = String::new();
        loop {
            let (event, span) = self.read()?;
            match event {
                Event::Text(content) => text.push_str(&content.xml10_content()),
                Event::CData(content) => text.push_str(&content.into_inner()),
                Event::GeneralRef(reference) => push_reference(&mut text, &reference.into_inner()),
                Event::End(_) => {
                    self.end = span;
                    return Ok(text);
                }
                Event::Eof => return Err(unexpected_eof(span)),
                Event::Start(start) => {
                    let nested = Tag {
                        start,
                        span,
                        empty: false,
                    };
                    self.skip(&nested)?;
                }
                _ => {}
            }
        }
    }

    /// An element and everything under it, kept as written.
    pub fn element(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<XmlElement, DocumentError> {
        let mut element = XmlElement {
            name: tag.name().to_owned(),
            attributes: tag.attributes(),
            children: Vec::new(),
        };
        if tag.empty {
            self.end = Span::new(tag.span.end, tag.span.end);
            return Ok(element);
        }
        element.children = self.children()?;
        Ok(element)
    }

    pub fn skip(
        &mut self,
        tag: &Tag<'a>,
    ) -> Result<(), DocumentError> {
        self.element(tag).map(|_| ())
    }

    fn children(&mut self) -> Result<Vec<XmlNode>, DocumentError> {
        let mut children = Vec::new();
        loop {
            let (event, span) = self.read()?;
            match event {
                Event::Start(start) => {
                    let tag = Tag {
                        start,
                        span,
                        empty: false,
                    };
                    children.push(XmlNode::Element(self.element(&tag)?));
                }
                Event::Empty(start) => {
                    let tag = Tag {
                        start,
                        span,
                        empty: true,
                    };
                    children.push(XmlNode::Element(self.element(&tag)?));
                }
                Event::Text(content) => push_text(&mut children, &content.xml10_content()),
                Event::GeneralRef(reference) => {
                    let mut resolved = String::new();
                    push_reference(&mut resolved, &reference.into_inner());
                    push_text(&mut children, &resolved);
                }
                Event::CData(content) => children.push(XmlNode::CData(content.into_inner().into_owned())),
                Event::Comment(content) => children.push(XmlNode::Comment(content.into_inner().into_owned())),
                Event::PI(content) => {
                    children.push(XmlNode::ProcessingInstruction(content.into_inner().into_owned()));
                }
                Event::End(_) => {
                    self.end = span;
                    return Ok(children);
                }
                Event::Eof => return Err(unexpected_eof(span)),
                _ => {}
            }
        }
    }
}

/// Appends to the text node already at the end, so an entity reference does not split one.
fn push_text(
    children: &mut Vec<XmlNode>,
    text: &str,
) {
    match children.last_mut() {
        Some(XmlNode::Text(last)) => last.push_str(text),
        _ => children.push(XmlNode::Text(text.to_owned())),
    }
}

/// Resolves the five entities XML predefines and any character reference; anything else is a
/// reference to an entity no nodeset declares, so it is kept as it was written.
fn push_reference(
    text: &mut String,
    name: &str,
) {
    if let Some(character) = name.strip_prefix('#').and_then(resolve_character) {
        text.push(character);
        return;
    }
    match name {
        "amp" => text.push('&'),
        "lt" => text.push('<'),
        "gt" => text.push('>'),
        "quot" => text.push('"'),
        "apos" => text.push('\''),
        _ => {
            text.push('&');
            text.push_str(name);
            text.push(';');
        }
    }
}

fn resolve_character(digits: &str) -> Option<char> {
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hexadecimal) => u32::from_str_radix(hexadecimal, 16).ok()?,
        None => digits.parse().ok()?,
    };
    char::from_u32(code)
}

fn unexpected_eof(span: Span) -> DocumentError {
    DocumentError::Syntax {
        offset: span.start,
        message: "the file ends inside an element".to_owned(),
    }
}
