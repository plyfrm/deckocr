/// A word and its definition, if one was found.
#[derive(Debug, Clone)]
pub struct Word {
    /// The word as it should appear in text.
    pub text: TextWithRuby,
    /// The word's definition, if one was found.
    pub definition: Option<Definition>,
}

/// A word's definition and associated data.
#[derive(Debug, Clone)]
pub struct Definition {
    /// The word's spelling.
    pub spelling: String,
    /// The word's reading.
    pub reading: String,
    /// The word's frequency rank, if one was found.
    pub frequency: Option<u64>,
    /// The word's meanings.
    pub meanings: Vec<String>,

    /// The word's jpdb `vid` and `sid` if it was retrieved via the jpdb api.
    pub jpdb_vid_sid: Option<(u64, u64)>,
}

/// Text with furigana.
#[derive(Debug, Hash, Clone)]
pub struct TextWithRuby(pub Vec<TextFragment>);

/// A fragment of text, optionally with its associated furigana.
#[derive(Debug, Hash, Clone)]
pub struct TextFragment {
    pub text: String,
    pub ruby: Option<String>,
}

impl<F: Into<TextFragment>> FromIterator<F> for TextWithRuby {
    fn from_iter<T: IntoIterator<Item = F>>(iter: T) -> Self {
        TextWithRuby(iter.into_iter().map(|f| f.into()).collect())
    }
}

impl<F: Into<TextFragment>> From<F> for TextWithRuby {
    fn from(value: F) -> Self {
        Self(vec![value.into()])
    }
}

impl From<(String, Option<String>)> for TextFragment {
    fn from(value: (String, Option<String>)) -> Self {
        TextFragment {
            text: value.0,
            ruby: value.1,
        }
    }
}

impl From<(String, String)> for TextFragment {
    fn from(value: (String, String)) -> Self {
        (value.0, Some(value.1)).into()
    }
}

impl From<String> for TextFragment {
    fn from(value: String) -> Self {
        (value, None).into()
    }
}
