use crate::*;
use std::{
    fs::File,
    io::{stdin, stdout, BufReader, Read, Write},
    iter::Peekable,
    path::PathBuf,
};

// NOTE: I spend a bunch of time on the below trying to make this work at compile time with macros
// but it didn't really work out how I expected, and I don't think it's possible ... idk
// maybe it is possible and I just need to look at it again

// I want like a string literal type in keyword, because I know it's a fixed number of strings
// I also don't want to do some dumb ass shit like mis-spell something on accident
// was also thinking that a macro could maybe help with compile time check that way I can do
// something like Token::Keyword(class!()) or keyword_token!("classs") - in this case I'd get
// a compiler error because "classs" isn't a keyword
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // so keyword
    Keyword(String),
    Symbol(String),
    IntegerConstant(usize),
    StringConstant(String),
    Identifier(String),
}

impl Token {
    pub fn as_xml_string(&self) -> String {
        match self {
            Token::Symbol(ident) => match ident.as_str().chars().nth(0) {
                Some('<') => "<symbol> &lt; </symbol>\n".to_string(),
                Some('>') => "<symbol> &gt; </symbol>\n".to_string(),
                Some('"') => "<symbol> &quot; </symbol>\n".to_string(),
                Some('&') => "<symbol> &amp; </symbol>\n".to_string(),
                _ => {
                    format!("<symbol> {} </symbol>\n", ident)
                }
            },
            Token::Keyword(keyword) => {
                format!("<keyword> {} </keyword>\n", keyword)
            }
            Token::IntegerConstant(ident) => {
                format!("<integerConstant> {} </integerConstant>\n", ident)
            }
            Token::StringConstant(ident) => {
                format!("<stringConstant> {} </stringConstant>\n", ident)
            }
            Token::Identifier(ident) => {
                format!("<identifier> {} </identifier>\n", ident)
            }
        }
    }
    // could just return &String - I don't need to clone a new one, because I
    // only really ever read this value
    pub fn get_value(&self) -> String {
        match self {
            Token::Symbol(ident) => match ident.as_str().chars().nth(0) {
                Some('<') => "lt;".to_string(),
                Some('>') => "gt;".to_string(),
                Some('"') => "&quot".to_string(),
                Some('&') => "&amp".to_string(),
                _ => ident.clone(),
            },
            Token::Keyword(keyword) => keyword.clone(),
            Token::IntegerConstant(ident) => ident.clone().to_string(),
            Token::StringConstant(ident) => ident.clone(),
            Token::Identifier(ident) => ident.clone(),
        }
    }
}

pub struct TokenIterator(Vec<Token>, usize);

impl Iterator for TokenIterator {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.len() > self.1 {
            let next = Some(self.0[self.1].clone());
            self.1 += 1;
            next
        } else {
            None
        }
    }
}

impl From<Vec<Token>> for TokenIterator {
    fn from(token_vec: Vec<Token>) -> Self {
        TokenIterator(token_vec, 0)
    }
}

/*
TODO: this class is kind of a mess...
ergonomically it's not great? I should look at that Rust API thing
maybe it actually has some good recomendations: https://rust-lang.github.io/api-guidelines/ <- these are the guidlines
 */
pub struct JackTokenizer {
    f: Option<BufReader<File>>,
}

impl JackTokenizer {
    pub fn new(p: PathBuf) -> Result<Self> {
        let f = BufReader::new(File::open(p)?);
        Ok(JackTokenizer { f: Some(f) })
    }

    // only have this, because repl...
    pub fn new_empty() -> Self {
        JackTokenizer { f: None }
    }

    // this method is one of the biggest cluster fucks. Need to look how I
    // can make simpler
    fn peek(b: &mut Peekable<impl Iterator<Item = IOResult>>) -> Result<char> {
        let next = b
            .peek()
            .ok_or_else(|| CatchAllError::Generic("thing random thing...".into()))?
            .as_ref()
            .map_err(|_| {
                let error_placeholder: anyhow::Error =
                    CatchAllError::Generic("this erorr instead".into()).into();
                error_placeholder
            });

        next.and_then(|inside| Ok(inside.clone().into()))
    }

    fn next(b: &mut impl Iterator<Item = IOResult>) -> Option<Result<char>> {
        b.next()
            .map(|v| v.map(|vv| vv as char).map_err(|err| err.into()))
    }

    fn tokens_from_string(trimmed_string: &str) -> Vec<Token> {
        // assuming the string will always be trimmed - shouls panic or bail if it isn't

        trace!("trimmed_string at start: {}", trimmed_string);
        let mut v = Vec::new();

        let possible_tokens = trimmed_string.split(" ");

        for possible_token in possible_tokens {
            // would this have been easier with regex or macro
            if matches!(
                possible_token,
                "class"
                    | "constructor"
                    | "method"
                    | "function"
                    | "int"
                    | "boolean"
                    | "char"
                    | "void"
                    | "var"
                    | "static"
                    | "let"
                    | "do"
                    | "if"
                    | "else"
                    | "while"
                    | "return"
                    | "true"
                    | "false"
                    | "null"
                    | "this"
                    | "field"
            ) {
                v.push(Token::Keyword(possible_token.to_string()));
            } else if let Ok(int) = trimmed_string.parse::<usize>() {
                v.push(Token::IntegerConstant(int))
            } else if trimmed_string.starts_with('\"') && trimmed_string.ends_with('\"') {
                let string_literal = trimmed_string.get(1..trimmed_string.len());
                match string_literal {
                    Some(string_literal) => {
                        v.push(Token::StringConstant(string_literal.to_string()))
                    }
                    None => panic!("string literal could not be parse"),
                }
            } else {
                v.push(Token::Identifier(possible_token.to_string()));
            }
        }

        v
    }

    fn tokenize_before_symbol(string: &mut Vec<char>, tokens: &mut Vec<Token>) {
        let trimmed_string = string.iter().collect::<String>();
        let trimmed_string = trimmed_string.trim();
        if !trimmed_string.is_empty() {
            let mut toks = Self::tokens_from_string(trimmed_string);
            tokens.append(toks.as_mut());
            string.clear();
        }
    }

    pub fn run(self, jack_string: Option<impl Into<String>>) -> Result<Vec<Token>> {
        // take a String or take Bytes<BufReader<File>> <- both of these are iterators

        match jack_string {
            Some(string) => {
                let string: String = string.into();
                let bb = string.as_bytes();
                let map_fn = |u8val: &u8| -> IOResult { return Ok(u8val.clone()) };
                let pp = bb.iter().peekable().map(map_fn);
                Self::tokenize(pp)
            }
            None => {
                let JackTokenizer { f: bytes } = self;

                let bytes = bytes
                    .expect("a file to exist for jack toenizer")
                    .bytes()
                    .peekable();
                Self::tokenize(bytes)
            }
        }
    }

    pub fn tokenize(input: impl IntoIterator<Item = IOResult>) -> Result<Vec<Token>> {
        let (mut tokens, mut string) = (Vec::new(), Vec::new());

        let mut bytes = input.into_iter().peekable();

        loop {
            let next = Self::next(&mut bytes);

            match next {
                None => {
                    // we're done if the next thing is None

                    trace!("string at time of return: {:?}", string);
                    // found the issue - basically not using things if they are left over?
                    // also there is the question of is this even right?
                    // like 1 + 2 + 3 -> probably isn't suppose to tokenize correctly?
                    // but 1 + 2 + 3; -> is suppose to tokenize correctly
                    // but for parsing I need 1 + 2 + 3 to give me everything

                    // print!("string? {:?}", string);
                    Self::tokenize_before_symbol(&mut string, &mut tokens);
                    return Ok(tokens);
                }
                Some(character) => {
                    match character {
                        Err(e) => {
                            panic!("don't know what to do with error: {}", e);
                        }
                        Ok(character) => match character {
                            '{' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("{".into()))
                            }
                            '}' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("}".into()));
                            }
                            '(' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("(".into()));
                            }
                            ')' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol(")".into()));
                            }
                            '[' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("[".into()));
                            }
                            ']' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("]".into()));
                            }
                            '.' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol(".".into()));
                            }
                            ',' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol(",".into()));
                            }
                            ';' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol(";".into()))
                            }
                            '+' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("+".into()));
                            }
                            '-' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("-".into()));
                            }
                            '~' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("~".into()));
                            }
                            '*' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("*".into()));
                            }
                            '/' => {
                                // this could be division
                                trace!("looking a '/'");
                                if let Ok('/') = Self::peek(&mut bytes) {
                                    // this is a comment so just skip the line
                                    // or just skip the rest of this

                                    trace!("found a comment '//'");
                                    // read until end of line
                                    while let Some(Ok(c)) = Self::next(&mut bytes) {
                                        string.push(c);
                                        if c == '\n' {
                                            // let bb = string.join(' '); // this doesn't work
                                            let bb = string.iter().collect::<String>();
                                            trace!("line: {:?}", &bb);
                                            string.clear();
                                            break;
                                        }
                                    }
                                } else if let Ok('*') = Self::peek(&mut bytes) {
                                    // TODO: test if multiline comments actually work
                                    // read until you see */
                                    trace!("found a comment '/*'");
                                    while let Some(Ok(c)) = Self::next(&mut bytes) {
                                        let char_after_next = Self::peek(&mut bytes);
                                        string.push(c);
                                        // if c == '*' && matches!(char_after_next, Ok('/')) {
                                        if c == '*' && matches!(char_after_next, Ok('/')) {
                                            // let bb = string.join(' '); // this doesn't work
                                            string.push(Self::next(&mut bytes).unwrap().unwrap());
                                            let bb = string.iter().collect::<String>();
                                            trace!("line: {:?}", &bb);
                                            string.clear();
                                            break;
                                        }
                                    }
                                } else {
                                    // it must be division
                                    trace!("found division: add '/' to tokens");
                                    tokens.push(Token::Symbol("/".into()));
                                    string.clear();
                                }
                            }
                            '&' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("&".into()));
                            }
                            '|' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("|".into()));
                            }
                            '<' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("<".into()));
                            }
                            '>' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol(">".into()));
                            }
                            '=' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("=".into()));
                            }
                            '_' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                tokens.push(Token::Symbol("_".into()));
                            }
                            ' ' | '\n' => {
                                // NOTE: I don't think I'll want this for newlines all the time actually.........

                                // so what's happening if this is white space?
                                // well it could just be white space at the beginning of a line
                                // or the end of a line or it could be the case that the string
                                // we've been building has a valid token inside of it? What is
                                // the best way to check that?

                                // this should be a trimmed string up to a white space -
                                // it could just be an empty string for example
                                // let trimmed_string = string.iter().collect::<String>();
                                // let trimmed_string = trimmed_string.trim();
                                // // dbg!(&tokens);
                                // if !trimmed_string.is_empty() {
                                //     let mut toks = Self::tokens_from_string(trimmed_string);
                                //     dbg!(&toks);
                                //     tokens.append(toks.as_mut());
                                //     string.clear();
                                // }

                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                // function for figuring out what the possible thing is
                                // then turning it into the correct token
                            }
                            '"' => {
                                Self::tokenize_before_symbol(&mut string, &mut tokens);
                                // kinda not super obvious that the string will be cleared...
                                // but it is - should make that explicit

                                // I don't want the quoted StringConstant
                                // "" is a valid string constant

                                // tokenize_before_symbol handles StringConstant's but it doesn't
                                // really work because parsing it to that point is wrong..

                                trace!("FOUND A STRING");
                                // STOPPED HERE: peek is panicing because I'm unwrapping
                                while let Ok(string_constant_char) = Self::peek(&mut bytes) {
                                    if string_constant_char == '"' {
                                        // found ned of string
                                        Self::next(&mut bytes); // move past final '"'
                                        break;
                                    }
                                    string.push(string_constant_char);
                                    Self::next(&mut bytes);
                                }
                                tokens.push(Token::StringConstant(string.iter().collect()));
                                string.clear();
                            }
                            _ => {
                                // at this point we are dealing with not a symbol
                                // not a comment so we must be dealing with actual characters
                                string.push(character);
                                // since we went through all of the symbols we must be
                                // on something else then
                            }
                        },
                    }
                }
            }
        }
    }

    fn write_to_xml(tokens: Vec<Token>, output_file: Option<&PathBuf>) -> Result<()> {
        let mut xml_f = match output_file {
            Some(file_name) => {
                trace!("created (or truncating) to: {:?}", &output_file);
                File::create(PathBuf::from(file_name))
            }
            None => {
                trace!("created (or truncating) to: {:?}", "testing.xml");
                File::create(PathBuf::from("testing.xml"))
            }
        }?;

        xml_f.write("<tokens>\n".as_bytes())?;
        // just an idea is a warp macro basically you give it an
        // identifier then it writes that identifier and evalutaes the
        // value between it for xml_write!(identifier) -> "<identifer> {eval identifier} </identifier>"
        // TODO: do the above
        for token in tokens {
            match token {
                Token::Symbol(ident) => match ident.as_str().chars().nth(0) {
                    Some('<') => {
                        xml_f.write("<symbol> &lt; </symbol>\n".as_bytes())?;
                    }
                    Some('>') => {
                        xml_f.write("<symbol> &gt; </symbol>\n".as_bytes())?;
                    }
                    Some('"') => {
                        xml_f.write("<symbol> &quot; </symbol>\n".as_bytes())?;
                    }
                    Some('&') => {
                        xml_f.write("<symbol> &amp; </symbol>\n".as_bytes())?;
                    }
                    _ => {
                        xml_f.write(format!("<symbol> {} </symbol>\n", ident).as_bytes())?;
                    }
                },
                Token::Keyword(keyword) => {
                    xml_f.write(format!("<keyword> {} </keyword>\n", keyword).as_bytes())?;
                }
                Token::IntegerConstant(ident) => {
                    xml_f.write(
                        format!("<integerConstant> {} </integerConstant>\n", ident).as_bytes(),
                    )?;
                }
                Token::StringConstant(ident) => {
                    xml_f.write(
                        format!("<stringConstant> {} </stringConstant>\n", ident).as_bytes(),
                    )?;
                }
                Token::Identifier(ident) => {
                    xml_f.write(format!("<identifier> {} </identifier>\n", ident).as_bytes())?;
                }
            }
        }
        xml_f.write("</tokens>\n".as_bytes())?;
        Ok(())
    }
}

pub fn main(jack_file_name: PathBuf, output_file_name: Option<PathBuf>) -> Result<()> {
    /*
    TODOS:
    - what should main function actually do?
    - remove all the warnings
    - would be good to write some unit tests as this would actually be useful for this
    - also would be good to refactor this code as it's not very easy to understand I think
    - Remove all these comments as they're kinda useless
     */

    trace!("tokenizer starting");

    let jack_tokenizer = JackTokenizer::new(PathBuf::from(jack_file_name.clone()))?;
    // let all_tokens = jack_tokenizer.tokenize()?;
    let t: Option<String> = None;
    let all_tokens = jack_tokenizer.run(t)?;
    trace!("all tokens for: {:?}", jack_file_name);
    dbg!(&all_tokens);
    // can reuse pathbuf with referecne here - I don't think there's any reason
    // it needs to be comsumed
    JackTokenizer::write_to_xml(all_tokens, output_file_name.as_ref())?;

    Ok(())
}

pub fn repl() -> Result<()> {
    // what should this do? I should read a line from stdin

    println!("Jack Repl: 0.0.1 (Rust Version)");

    let mut character_stream = String::new();
    loop {
        stdout().write("ƛ ".as_bytes())?;
        stdout().flush()?;
        stdin().read_line(&mut character_stream)?;
        // stdin().read_to_string(&mut character_stream)?;
        dbg!(&character_stream);
        // TODO: tokenize from input - probably good to think about how mulit-line things will
        // work as well
        let b = JackTokenizer::new_empty();
        let toks = b.run(Some(character_stream.clone()));

        dbg!(toks?);

        if character_stream.as_str() == "exit\n" {
            break;
        }
        character_stream.clear();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_test() {
        assert_eq!(Token::Symbol(")".into()), Token::Symbol(")".into()))
    }

    #[test]
    fn test_string_one() -> Result<()> {
        let parsed_value = JackTokenizer::new_empty().run(Some("1 + 2 + 3\n"))?;
        let expected_value = vec![
            Token::IntegerConstant(1),
            Token::Symbol("+".into()),
            Token::IntegerConstant(2),
            Token::Symbol("+".into()),
            Token::IntegerConstant(3),
        ];
        assert_eq!(parsed_value, expected_value);
        Ok(())
    }

    #[test]
    fn test_string_two() -> Result<()> {
        let parsed_value = JackTokenizer::new_empty().run(Some("1 + 2 + 3"))?;

        /*
        - this is just a thought, but I could have a macro that returns the tokens in a vector
        tok!(1 + 2 + 3); would return the below vector
        */
        let expected_value = vec![
            Token::IntegerConstant(1),
            Token::Symbol("+".into()),
            Token::IntegerConstant(2),
            Token::Symbol("+".into()),
            Token::IntegerConstant(3),
        ];
        assert_eq!(parsed_value, expected_value);
        Ok(())
    }

    #[test]
    fn test_hello_jack_file() -> Result<()> {
        let parsed_value = JackTokenizer::new("testing/personal_testing/hello.jack".into())?
            .run(Option::<String>::None)?;
        let expected_value = vec![
            Token::Keyword("class".into()),
            Token::Symbol("+".into()),
            Token::IntegerConstant(2),
            Token::Symbol("+".into()),
            Token::IntegerConstant(3),
        ];
        dbg!(parsed_value);
        // assert_eq!(parsed_value, expected_value);
        Ok(())
    }
}
