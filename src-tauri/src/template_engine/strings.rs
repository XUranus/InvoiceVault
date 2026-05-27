/// Shared string pool with deduplication.
/// Maintains original template strings at their original indices,
/// then appends new strings added during data binding.
#[derive(Debug, Clone)]
pub struct SharedStringPool {
    strings: Vec<String>,
    original_count: usize,
}

impl SharedStringPool {
    pub fn from_existing(strings: Vec<String>) -> Self {
        let original_count = strings.len();
        Self {
            strings,
            original_count,
        }
    }

    /// Get the index of an existing string, or insert it and return the new index.
    pub fn get_or_insert(&mut self, value: &str) -> usize {
        if let Some(idx) = self.strings.iter().position(|s| s == value) {
            return idx;
        }
        let idx = self.strings.len();
        self.strings.push(value.to_owned());
        idx
    }

    /// Get the index of an existing string. Returns None if not found.
    pub fn index_of(&self, value: &str) -> Option<usize> {
        self.strings.iter().position(|s| s == value)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.strings.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Whether the pool was extended beyond the original template strings.
    pub fn is_extended(&self) -> bool {
        self.strings.len() != self.original_count
    }

    /// Render the full xl/sharedStrings.xml document.
    pub fn to_xml(&self) -> String {
        let count = self.strings.len();
        let mut xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{count}" uniqueCount="{count}">"#
        );
        for s in &self.strings {
            xml.push_str("<si><t xml:space=\"preserve\">");
            xml.push_str(&xml_escape(s));
            xml.push_str("</t></si>");
        }
        xml.push_str("</sst>");
        xml
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dedup() {
        let mut pool = SharedStringPool::from_existing(vec!["hello".into(), "world".into()]);
        assert_eq!(pool.get_or_insert("hello"), 0);
        assert_eq!(pool.get_or_insert("world"), 1);
        assert_eq!(pool.get_or_insert("new"), 2);
        assert_eq!(pool.len(), 3);
        assert!(pool.is_extended());
    }

    #[test]
    fn test_index_of() {
        let pool = SharedStringPool::from_existing(vec!["a".into(), "b".into()]);
        assert_eq!(pool.index_of("a"), Some(0));
        assert_eq!(pool.index_of("b"), Some(1));
        assert_eq!(pool.index_of("c"), None);
    }

    #[test]
    fn test_to_xml() {
        let pool = SharedStringPool::from_existing(vec!["hello".into(), "w<o>&rld".into()]);
        let xml = pool.to_xml();
        assert!(xml.contains("<si><t xml:space=\"preserve\">hello</t></si>"));
        assert!(xml.contains("<si><t xml:space=\"preserve\">w&lt;o&gt;&amp;rld</t></si>"));
        assert!(xml.contains(r#"count="2""#));
    }
}
