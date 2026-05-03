use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ObjectId(String);

impl ObjectId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PageId(ObjectId);

impl PageId {
    pub(crate) const UNTITLED_FALLBACK: &'static str = "untitled";

    pub(crate) fn new(id: ObjectId) -> Self {
        Self(id)
    }

    pub(crate) fn from_string(value: impl Into<String>) -> Self {
        Self(ObjectId::new(value))
    }

    pub(crate) fn untitled_fallback() -> Self {
        Self::from_string(Self::UNTITLED_FALLBACK)
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}
