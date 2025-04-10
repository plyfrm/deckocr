#[derive(Debug, Clone)]
pub struct Word {
    pub text: TextWithRuby,
    pub definition: Option<Definition>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub spelling: String,
    pub reading: String,
    pub frequency: Option<u64>,
    pub meanings: Vec<String>,

    pub jpdb_vid_sid: Option<(u64, u64)>,
}

#[derive(Debug, Hash, Clone)]
pub struct TextWithRuby(pub Vec<TextFragment>);

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
