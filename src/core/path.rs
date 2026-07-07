//! Path addressing — a list of segments used to navigate the Value tree.

/// A single segment in a navigation path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum PathSegment {
    /// Navigate into an object by key.
    Key(String),
    /// Navigate into an array by index.
    Index(usize),
}

impl From<String> for PathSegment {
    fn from(s: String) -> Self {
        Self::Key(s)
    }
}

impl From<&str> for PathSegment {
    fn from(s: &str) -> Self {
        Self::Key(s.to_owned())
    }
}

impl From<usize> for PathSegment {
    fn from(i: usize) -> Self {
        Self::Index(i)
    }
}

/// A path through the Value tree, represented as a list of segments.
///
/// For example, `["server", "port"]` navigates to the `port` field
/// inside the `server` object. `["items", 0]` navigates to the first
/// element of the `items` array.
#[allow(dead_code)]
pub type Path = Vec<PathSegment>;

/// Convenience macros for building paths inline.
///
/// ```
/// use confui::core::path;
///
/// let p = path!["server", "port"];
/// assert_eq!(p.len(), 2);
/// ```
#[macro_export]
macro_rules! path {
    () => {
        $crate::core::Path::new()
    };
    ( $( $segment:expr ),+ $(,)? ) => {
        vec![$(
            $crate::core::PathSegment::from($segment)
        ),*]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_from_str() {
        let p: Path = vec!["a".into(), "b".into(), "c".into()];
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn path_from_usize() {
        let p: Path = vec![0usize.into(), 1usize.into()];
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn path_macro() {
        let p = path!["server", "port"];
        assert_eq!(
            p,
            vec![
                PathSegment::Key("server".into()),
                PathSegment::Key("port".into())
            ]
        );

        let p = path!["items", 0, "name"];
        assert_eq!(
            p,
            vec![
                PathSegment::Key("items".into()),
                PathSegment::Index(0),
                PathSegment::Key("name".into()),
            ]
        );
    }
}
