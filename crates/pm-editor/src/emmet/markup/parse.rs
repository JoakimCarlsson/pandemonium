//! Reading a markup abbreviation into a tree of elements and groups.

/// The most times one `*n` may repeat a node.
const MAX_REPEAT: usize = 500;

/// One attribute written in `[...]`.
#[derive(Clone, Debug)]
pub(super) struct Attribute {
    /// The attribute name.
    pub name: String,
    /// The value, or nothing when the attribute was written bare.
    pub value: Option<String>,
}

/// One written element, with what it was given and what lies inside it.
#[derive(Clone, Debug, Default)]
pub(super) struct Element {
    /// The tag name, or nothing when the parent decides it.
    pub name: Option<String>,
    /// The `#id`.
    pub id: Option<String>,
    /// Every `.class`, in order.
    pub classes: Vec<String>,
    /// The `[...]` attributes, in order.
    pub attributes: Vec<Attribute>,
    /// The `{text}`.
    pub text: Option<String>,
    /// The `*n`, when one was written.
    pub repeat: Option<usize>,
    /// The elements after `>`.
    pub children: Vec<Node>,
}

/// A part of an abbreviation: one element or a parenthesised run of siblings.
#[derive(Clone, Debug)]
pub(super) enum Node {
    /// A single element.
    Element(Element),
    /// Siblings in `( ... )`, repeated as a whole.
    Group {
        /// The siblings in the group.
        items: Vec<Node>,
        /// The `*n` after the parenthesis, when one was written.
        repeat: Option<usize>,
    },
}

impl Node {
    /// Puts `children` inside this node; a group cannot hold children, so nothing comes back for one.
    fn adopt(&mut self, children: Vec<Node>) -> Option<()> {
        match self {
            Node::Element(element) => {
                element.children = children;
                Some(())
            }
            Node::Group { .. } => None,
        }
    }

    /// Sets how many times this node is written.
    fn repeat_by(&mut self, times: usize) {
        match self {
            Node::Element(element) => element.repeat = Some(times),
            Node::Group { repeat, .. } => *repeat = Some(times),
        }
    }
}

/// The nodes `abbreviation` is made of, or nothing when it is not a markup abbreviation.
pub(super) fn parse(abbreviation: &str) -> Option<Vec<Node>> {
    let mut parser = Parser {
        characters: abbreviation.chars().collect(),
        at: 0,
    };
    let nodes = parser.level(0)?;
    parser.peek().is_none().then_some(nodes)
}

/// A cursor over the characters of an abbreviation.
struct Parser {
    /// The abbreviation.
    characters: Vec<char>,
    /// The position of the next character to read.
    at: usize,
}

impl Parser {
    /// The next character, left in place.
    fn peek(&self) -> Option<char> {
        self.characters.get(self.at).copied()
    }

    /// Reads the next character.
    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.at += 1;
        Some(character)
    }

    /// Reads the next character when it is `expected`.
    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Reads siblings joined by `+`, each with its `>` children, until a `)` or the end; a `^` ends the level and is left for the parent that climbs.
    fn level(&mut self, depth: usize) -> Option<Vec<Node>> {
        let mut nodes = Vec::new();
        loop {
            nodes.push(self.item()?);
            if self.eat('>') {
                let children = self.level(depth + 1)?;
                nodes.last_mut()?.adopt(children)?;
                if self.peek() != Some('^') {
                    return Some(nodes);
                }
                self.at += 1;
            } else {
                match self.peek() {
                    Some('+') => {
                        self.at += 1;
                        continue;
                    }
                    Some('^') => {
                        if depth > 0 {
                            return Some(nodes);
                        }
                        self.at += 1;
                    }
                    _ => return Some(nodes),
                }
            }
            while self.peek() == Some('^') {
                if depth > 0 {
                    return Some(nodes);
                }
                self.at += 1;
            }
            self.eat('+');
            if matches!(self.peek(), None | Some(')')) {
                return Some(nodes);
            }
        }
    }

    /// Reads one element or group with its `*n`.
    fn item(&mut self) -> Option<Node> {
        let mut node = if self.eat('(') {
            let items = self.level(0)?;
            if !self.eat(')') {
                return None;
            }
            Node::Group {
                items,
                repeat: None,
            }
        } else {
            Node::Element(self.element()?)
        };
        if self.eat('*') {
            node.repeat_by(self.number()?);
        }
        Some(node)
    }

    /// Reads a name followed by any `#id`, `.class`, `[attributes]` and `{text}`.
    fn element(&mut self) -> Option<Element> {
        let mut element = Element::default();
        if self.peek().is_some_and(char::is_alphabetic) {
            element.name = Some(self.name());
        }
        let mut written = element.name.is_some();
        loop {
            match self.peek() {
                Some('#') => {
                    self.at += 1;
                    element.id = Some(self.word()?);
                }
                Some('.') => {
                    self.at += 1;
                    element.classes.push(self.word()?);
                }
                Some('[') => {
                    self.at += 1;
                    element.attributes.extend(self.attributes()?);
                }
                Some('{') => {
                    self.at += 1;
                    element.text = Some(self.text()?);
                }
                _ => break,
            }
            written = true;
        }
        written.then_some(element)
    }

    /// Reads a tag name.
    fn name(&mut self) -> String {
        let mut name = String::new();
        while let Some(character) = self.peek() {
            if !(character.is_alphanumeric() || matches!(character, '-' | ':')) {
                break;
            }
            name.push(character);
            self.at += 1;
        }
        name
    }

    /// Reads an id or class, which may hold item numbering.
    fn word(&mut self) -> Option<String> {
        let mut word = String::new();
        while let Some(character) = self.peek() {
            let accepted = character.is_alphanumeric()
                || matches!(character, '-' | '_' | '$')
                || (character == '@' && word.ends_with('$'));
            if !accepted {
                break;
            }
            word.push(character);
            self.at += 1;
        }
        (!word.is_empty()).then_some(word)
    }

    /// Reads the attributes up to the closing `]`.
    fn attributes(&mut self) -> Option<Vec<Attribute>> {
        let mut attributes = Vec::new();
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.at += 1;
            }
            if self.eat(']') {
                return Some(attributes);
            }
            let mut name = String::new();
            while let Some(character) = self.peek() {
                if character.is_whitespace() || matches!(character, '=' | ']') {
                    break;
                }
                name.push(character);
                self.at += 1;
            }
            if name.is_empty() {
                return None;
            }
            let value = if self.eat('=') {
                Some(self.attribute_value()?)
            } else {
                None
            };
            attributes.push(Attribute { name, value });
        }
    }

    /// Reads a quoted value, or a bare one up to a space or the closing `]`.
    fn attribute_value(&mut self) -> Option<String> {
        let mut value = String::new();
        if let Some(quote @ ('"' | '\'')) = self.peek() {
            self.at += 1;
            loop {
                let character = self.bump()?;
                if character == quote {
                    return Some(value);
                }
                value.push(character);
            }
        }
        while let Some(character) = self.peek() {
            if character.is_whitespace() || character == ']' {
                break;
            }
            value.push(character);
            self.at += 1;
        }
        Some(value)
    }

    /// Reads text up to the `}` that closes it, braces inside it pairing off.
    fn text(&mut self) -> Option<String> {
        let mut text = String::new();
        let mut depth = 0;
        loop {
            let character = self.bump()?;
            match character {
                '{' => depth += 1,
                '}' if depth == 0 => return Some(text),
                '}' => depth -= 1,
                _ => {}
            }
            text.push(character);
        }
    }

    /// Reads the count after `*`.
    fn number(&mut self) -> Option<usize> {
        let mut digits = String::new();
        while let Some(character) = self.peek().filter(char::is_ascii_digit) {
            digits.push(character);
            self.at += 1;
        }
        let times: usize = digits.parse().ok()?;
        (1..=MAX_REPEAT).contains(&times).then_some(times)
    }
}
