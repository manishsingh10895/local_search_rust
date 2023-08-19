use crate::snowball;

// Lexer should contain the parsed document, doesn't modify
#[derive(Debug)]
pub struct Lexer<'a> {
    content: &'a [char],
}

impl<'a> Lexer<'a> {
    pub fn new(content: &'a [char]) -> Self {
        Self { content }
    }

    fn chop(&mut self, n: usize) -> &'a [char] {
        let token = &self.content[0..n];
        self.content = &self.content[n..];

        token
    }

    fn chop_while<P>(&mut self, mut predicate: P) -> &'a [char]
    where
        P: FnMut(&char) -> bool,
    {
        let mut n = 0;
        while n < self.content.len() && predicate(&self.content[n]) {
            n += 1;
        }

        return self.chop(n);
    }

    fn next_token(&mut self) -> Option<String> {
        // trim whitespaces from left
        self.trim_left();

        if self.content.len() == 0 {
            return None;
        }

        // Lex alphabetic words
        if self.content[0].is_alphabetic() {
            let term = self
                .chop_while(|x| x.is_alphabetic())
                .iter()
                .map(|x| x.to_ascii_lowercase())
                .collect::<String>();

            // stemming of the term directly in lexer itself
            let mut env = snowball::SnowballEnv::create(&term);
            snowball::algorithms::english_stemmer::stem(&mut env);
            let stemmed = env.get_current().to_string();

            return Some(stemmed);
        }

        //lex numbers
        if self.content[0].is_numeric() {
            return Some(self.chop_while(|x| x.is_numeric()).iter().collect());
        }

        // Unhandled tokens
        // proceed to next token for next iteration
        //
        Some(self.chop(1).iter().collect())
    }

    fn trim_left(&mut self) {
        while self.content.len() > 0 && self.content[0].is_whitespace() {
            self.content = &self.content[1..];
        }
    }
}

/// Iterator for the Lexer to iterate over
/// generated token
impl<'a> Iterator for Lexer<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}
