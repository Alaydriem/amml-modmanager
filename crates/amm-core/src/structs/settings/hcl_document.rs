use hcl::eval::{Context, Evaluate};
use hcl_edit::expr::Expression;
use hcl_edit::structure::{Attribute, Block, Body};
use hcl_edit::{Decor, Decorate, Ident};

use crate::services::settings::HclPath;
use crate::structs::settings::ConfigWarning;

/// A config file held as an `hcl-edit` syntax tree, so a write keeps comments, blank lines, order and every item
/// the schema does not name. Edits return a new document.
#[derive(Debug, Clone)]
pub struct HclDocument {
    body: Body,
}

impl HclDocument {
    const INDENT: &'static str = "  ";

    pub fn new(text: &str) -> Result<HclDocument, ConfigWarning> {
        text.parse::<Body>().map(|body| Self { body }).map_err(|_| ConfigWarning::Unparseable)
    }

    pub fn empty() -> HclDocument {
        Self { body: Body::new() }
    }

    /// The value of the attribute at a dotted path through unlabelled blocks. `None` when the path is bad, a
    /// block or the attribute is missing, or the expression does not evaluate without variables.
    pub fn get(&self, path: &str) -> Option<hcl::Value> {
        let mut segments = HclPath::split(path)?.peekable();
        let mut body = &self.body;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                let expression = hcl::Expression::from(body.get_attribute(segment)?.value.clone());
                return expression.evaluate(&Context::new()).ok();
            }
            body = &Self::block(body, segment)?.body;
        }
        None
    }

    /// A copy with the attribute at `path` set to `value`. Missing unlabelled blocks are created. An attribute
    /// where the path needs a block is replaced, in its place and with its comments, by a block. The copy is
    /// unchanged when the path is bad.
    pub fn with(&self, path: &str, value: &hcl::Value) -> HclDocument {
        let Some(segments) = HclPath::split(path) else {
            return self.clone();
        };
        let mut body = self.body.clone();
        let expression = Expression::from(hcl::Expression::from(value.clone()));
        if Self::set(&mut body, segments, expression) { Self { body } } else { self.clone() }
    }

    pub fn text(&self) -> String {
        self.body.to_string()
    }

    fn block<'a>(body: &'a Body, ident: &str) -> Option<&'a Block> {
        body.blocks().find(|block| block.has_ident(ident) && !block.is_labeled())
    }

    fn set<'a>(root: &mut Body, segments: impl Iterator<Item = &'a str>, value: Expression) -> bool {
        let mut segments = segments.peekable();
        let mut body = root;
        let mut depth = 0;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                return Self::set_attribute(body, segment, value, depth);
            }
            if body.has_attribute(segment) && !Self::attribute_to_block(body, segment, depth) {
                return false;
            }
            if Self::block(body, segment).is_none() {
                let Ok(ident) = Ident::try_new(segment) else {
                    return false;
                };
                let mut block = Block::new(ident);
                block.body.decorate(Decor::new("", Self::INDENT.repeat(depth)));
                block.decorate(Decor::new(Self::new_item_prefix(body, depth), ""));
                body.push(block);
            }
            let Some(block) = body.blocks_mut().find(|block| block.has_ident(segment) && !block.is_labeled()) else {
                return false;
            };
            body = &mut block.body;
            depth += 1;
        }
        false
    }

    // Replaces the attribute `key` by an empty unlabelled block of the same name at the same place, keeping the
    // attribute's comments. False when the name is not a valid block name.
    fn attribute_to_block(body: &mut Body, key: &str, depth: usize) -> bool {
        let Ok(ident) = Ident::try_new(key) else {
            return false;
        };
        let found = body.iter().position(|item| item.as_attribute().is_some_and(|attribute| attribute.has_key(key)));
        let Some(index) = found else {
            return false;
        };
        let removed = body.remove(index);
        let mut block = Block::new(ident);
        block.body.decorate(Decor::new("", Self::INDENT.repeat(depth)));
        block.decorate(removed.decor().clone());
        body.insert(index, block);
        true
    }

    fn set_attribute(body: &mut Body, key: &str, mut value: Expression, depth: usize) -> bool {
        if let Some(mut attribute) = body.get_attribute_mut(key) {
            value.decorate(attribute.value_mut().decor().clone());
            *attribute.value_mut() = value;
            return true;
        }
        if body.has_blocks(key) {
            return false;
        }
        let Ok(ident) = Ident::try_new(key) else {
            return false;
        };
        let mut attribute = Attribute::new(ident, value);
        attribute.decorate(Decor::new(Self::new_item_prefix(body, depth), ""));
        body.set_prefer_oneline(false);
        body.push(attribute);
        true
    }

    // Comments after the last item of the file belong to the body, so they move in front of an appended item
    // to keep their place in the text.
    fn new_item_prefix(body: &mut Body, depth: usize) -> String {
        let indent = Self::INDENT.repeat(depth);
        if depth > 0 {
            return indent;
        }
        let trailing = body.decor().suffix().map(|suffix| suffix.to_string()).filter(|text| !text.trim().is_empty());
        let Some(mut prefix) = trailing else {
            return indent;
        };
        body.decor_mut().set_suffix("");
        if !prefix.ends_with('\n') {
            prefix.push('\n');
        }
        prefix
    }
}
