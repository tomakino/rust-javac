// Origin: com/sun/tools/javac/parser/Tokens.java @ 982d53c (jdk-17.0.19-ga)

//! A class that defines codes/utilities for Java source tokens returned from
//! lexical analysis.
//!
//! <p><b>This is NOT part of any supported API.
//! If you write code that depends on this, you do so at your own risk.
//! This code and its internal interfaces are subject to change or
//! deletion without notice.</b>
//!
//! PORTING_RULES:
//! - TokenKind keeps the upstream names verbatim (allow(non_camel_case_types))
//!   and the upstream name/tag table recorded from the token oracle (114 values);
//! - the Java Token/NamedToken/StringToken/NumericToken hierarchy becomes one
//!   Token struct with the TokenValue enum; the constructors keep the upstream
//!   (kind tag) assertions;
//! - Comment is a trait implemented by the tokenizer comment classes
//!   (java_tokenizer.rs / javadoc_tokenizer.rs); the comment list uses the
//!   ported immutable List<Comment>;
//! - Tokens.instance(Context) is injected in Rust (Names has no Context
//!   registration in the port): Tokens::new(Rc<Names>) keeps the upstream
//!   keyword table; the Context-based instance is a registered GAP (TODO(P6)).

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use javac_strings::JavaString;

use crate::util::list::List;
use crate::util::name::Name;
use crate::util::names::Names;

/// Java: Tokens.Token.Tag
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenTag {
    DEFAULT,
    NAMED,
    STRING,
    NUMERIC,
}

/// This enum defines all tokens used by the javac scanner (114 values, the
/// upstream names kept verbatim).
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    EOF,
    ERROR,
    IDENTIFIER,
    ABSTRACT,
    ASSERT,
    BOOLEAN,
    BREAK,
    BYTE,
    CASE,
    CATCH,
    CHAR,
    CLASS,
    CONST,
    CONTINUE,
    DEFAULT,
    DO,
    DOUBLE,
    ELSE,
    ENUM,
    EXTENDS,
    FINAL,
    FINALLY,
    FLOAT,
    FOR,
    GOTO,
    IF,
    IMPLEMENTS,
    IMPORT,
    INSTANCEOF,
    INT,
    INTERFACE,
    LONG,
    NATIVE,
    NEW,
    PACKAGE,
    PRIVATE,
    PROTECTED,
    PUBLIC,
    RETURN,
    SHORT,
    STATIC,
    STRICTFP,
    SUPER,
    SWITCH,
    SYNCHRONIZED,
    THIS,
    THROW,
    THROWS,
    TRANSIENT,
    TRY,
    VOID,
    VOLATILE,
    WHILE,
    INTLITERAL,
    LONGLITERAL,
    FLOATLITERAL,
    DOUBLELITERAL,
    CHARLITERAL,
    STRINGLITERAL,
    TRUE,
    FALSE,
    NULL,
    UNDERSCORE,
    ARROW,
    COLCOL,
    LPAREN,
    RPAREN,
    LBRACE,
    RBRACE,
    LBRACKET,
    RBRACKET,
    SEMI,
    COMMA,
    DOT,
    ELLIPSIS,
    EQ,
    GT,
    LT,
    BANG,
    TILDE,
    QUES,
    COLON,
    EQEQ,
    LTEQ,
    GTEQ,
    BANGEQ,
    AMPAMP,
    BARBAR,
    PLUSPLUS,
    SUBSUB,
    PLUS,
    SUB,
    STAR,
    SLASH,
    AMP,
    BAR,
    CARET,
    PERCENT,
    LTLT,
    GTGT,
    GTGTGT,
    PLUSEQ,
    SUBEQ,
    STAREQ,
    SLASHEQ,
    AMPEQ,
    BAREQ,
    CARETEQ,
    PERCENTEQ,
    LTLTEQ,
    GTGTEQ,
    GTGTGTEQ,
    MONKEYS_AT,
    CUSTOM,
}

impl TokenKind {
    /// The upstream enum declaration order.
    pub fn values() -> &'static [TokenKind] {
        &[TokenKind::EOF, TokenKind::ERROR, TokenKind::IDENTIFIER, TokenKind::ABSTRACT, TokenKind::ASSERT, TokenKind::BOOLEAN, TokenKind::BREAK, TokenKind::BYTE, TokenKind::CASE, TokenKind::CATCH, TokenKind::CHAR, TokenKind::CLASS, TokenKind::CONST, TokenKind::CONTINUE, TokenKind::DEFAULT, TokenKind::DO, TokenKind::DOUBLE, TokenKind::ELSE, TokenKind::ENUM, TokenKind::EXTENDS, TokenKind::FINAL, TokenKind::FINALLY, TokenKind::FLOAT, TokenKind::FOR, TokenKind::GOTO, TokenKind::IF, TokenKind::IMPLEMENTS, TokenKind::IMPORT, TokenKind::INSTANCEOF, TokenKind::INT, TokenKind::INTERFACE, TokenKind::LONG, TokenKind::NATIVE, TokenKind::NEW, TokenKind::PACKAGE, TokenKind::PRIVATE, TokenKind::PROTECTED, TokenKind::PUBLIC, TokenKind::RETURN, TokenKind::SHORT, TokenKind::STATIC, TokenKind::STRICTFP, TokenKind::SUPER, TokenKind::SWITCH, TokenKind::SYNCHRONIZED, TokenKind::THIS, TokenKind::THROW, TokenKind::THROWS, TokenKind::TRANSIENT, TokenKind::TRY, TokenKind::VOID, TokenKind::VOLATILE, TokenKind::WHILE, TokenKind::INTLITERAL, TokenKind::LONGLITERAL, TokenKind::FLOATLITERAL, TokenKind::DOUBLELITERAL, TokenKind::CHARLITERAL, TokenKind::STRINGLITERAL, TokenKind::TRUE, TokenKind::FALSE, TokenKind::NULL, TokenKind::UNDERSCORE, TokenKind::ARROW, TokenKind::COLCOL, TokenKind::LPAREN, TokenKind::RPAREN, TokenKind::LBRACE, TokenKind::RBRACE, TokenKind::LBRACKET, TokenKind::RBRACKET, TokenKind::SEMI, TokenKind::COMMA, TokenKind::DOT, TokenKind::ELLIPSIS, TokenKind::EQ, TokenKind::GT, TokenKind::LT, TokenKind::BANG, TokenKind::TILDE, TokenKind::QUES, TokenKind::COLON, TokenKind::EQEQ, TokenKind::LTEQ, TokenKind::GTEQ, TokenKind::BANGEQ, TokenKind::AMPAMP, TokenKind::BARBAR, TokenKind::PLUSPLUS, TokenKind::SUBSUB, TokenKind::PLUS, TokenKind::SUB, TokenKind::STAR, TokenKind::SLASH, TokenKind::AMP, TokenKind::BAR, TokenKind::CARET, TokenKind::PERCENT, TokenKind::LTLT, TokenKind::GTGT, TokenKind::GTGTGT, TokenKind::PLUSEQ, TokenKind::SUBEQ, TokenKind::STAREQ, TokenKind::SLASHEQ, TokenKind::AMPEQ, TokenKind::BAREQ, TokenKind::CARETEQ, TokenKind::PERCENTEQ, TokenKind::LTLTEQ, TokenKind::GTGTEQ, TokenKind::GTGTGTEQ, TokenKind::MONKEYS_AT, TokenKind::CUSTOM]
    }

    /// The keyword/text of this token kind (Java: TokenKind.name, null -> None).
    pub fn name(&self) -> Option<&'static str> {
        match self {
            TokenKind::ABSTRACT => Some("abstract"),
            // The token oracle writes "name=-" both for a null name and for the
            // SUB token whose text is "-"; SUB is restored from Tokens.java
            // (recorded data-format limitation).
            TokenKind::SUB => Some("-"),
            TokenKind::ASSERT => Some("assert"),
            TokenKind::BOOLEAN => Some("boolean"),
            TokenKind::BREAK => Some("break"),
            TokenKind::BYTE => Some("byte"),
            TokenKind::CASE => Some("case"),
            TokenKind::CATCH => Some("catch"),
            TokenKind::CHAR => Some("char"),
            TokenKind::CLASS => Some("class"),
            TokenKind::CONST => Some("const"),
            TokenKind::CONTINUE => Some("continue"),
            TokenKind::DEFAULT => Some("default"),
            TokenKind::DO => Some("do"),
            TokenKind::DOUBLE => Some("double"),
            TokenKind::ELSE => Some("else"),
            TokenKind::ENUM => Some("enum"),
            TokenKind::EXTENDS => Some("extends"),
            TokenKind::FINAL => Some("final"),
            TokenKind::FINALLY => Some("finally"),
            TokenKind::FLOAT => Some("float"),
            TokenKind::FOR => Some("for"),
            TokenKind::GOTO => Some("goto"),
            TokenKind::IF => Some("if"),
            TokenKind::IMPLEMENTS => Some("implements"),
            TokenKind::IMPORT => Some("import"),
            TokenKind::INSTANCEOF => Some("instanceof"),
            TokenKind::INT => Some("int"),
            TokenKind::INTERFACE => Some("interface"),
            TokenKind::LONG => Some("long"),
            TokenKind::NATIVE => Some("native"),
            TokenKind::NEW => Some("new"),
            TokenKind::PACKAGE => Some("package"),
            TokenKind::PRIVATE => Some("private"),
            TokenKind::PROTECTED => Some("protected"),
            TokenKind::PUBLIC => Some("public"),
            TokenKind::RETURN => Some("return"),
            TokenKind::SHORT => Some("short"),
            TokenKind::STATIC => Some("static"),
            TokenKind::STRICTFP => Some("strictfp"),
            TokenKind::SUPER => Some("super"),
            TokenKind::SWITCH => Some("switch"),
            TokenKind::SYNCHRONIZED => Some("synchronized"),
            TokenKind::THIS => Some("this"),
            TokenKind::THROW => Some("throw"),
            TokenKind::THROWS => Some("throws"),
            TokenKind::TRANSIENT => Some("transient"),
            TokenKind::TRY => Some("try"),
            TokenKind::VOID => Some("void"),
            TokenKind::VOLATILE => Some("volatile"),
            TokenKind::WHILE => Some("while"),
            TokenKind::TRUE => Some("true"),
            TokenKind::FALSE => Some("false"),
            TokenKind::NULL => Some("null"),
            TokenKind::UNDERSCORE => Some("_"),
            TokenKind::ARROW => Some("->"),
            TokenKind::COLCOL => Some("::"),
            TokenKind::LPAREN => Some("("),
            TokenKind::RPAREN => Some(")"),
            TokenKind::LBRACE => Some("{"),
            TokenKind::RBRACE => Some("}"),
            TokenKind::LBRACKET => Some("["),
            TokenKind::RBRACKET => Some("]"),
            TokenKind::SEMI => Some(";"),
            TokenKind::COMMA => Some(","),
            TokenKind::DOT => Some("."),
            TokenKind::ELLIPSIS => Some("..."),
            TokenKind::EQ => Some("="),
            TokenKind::GT => Some(">"),
            TokenKind::LT => Some("<"),
            TokenKind::BANG => Some("!"),
            TokenKind::TILDE => Some("~"),
            TokenKind::QUES => Some("?"),
            TokenKind::COLON => Some(":"),
            TokenKind::EQEQ => Some("=="),
            TokenKind::LTEQ => Some("<="),
            TokenKind::GTEQ => Some(">="),
            TokenKind::BANGEQ => Some("!="),
            TokenKind::AMPAMP => Some("&&"),
            TokenKind::BARBAR => Some("||"),
            TokenKind::PLUSPLUS => Some("++"),
            TokenKind::SUBSUB => Some("--"),
            TokenKind::PLUS => Some("+"),
            TokenKind::STAR => Some("*"),
            TokenKind::SLASH => Some("/"),
            TokenKind::AMP => Some("&"),
            TokenKind::BAR => Some("|"),
            TokenKind::CARET => Some("^"),
            TokenKind::PERCENT => Some("%"),
            TokenKind::LTLT => Some("<<"),
            TokenKind::GTGT => Some(">>"),
            TokenKind::GTGTGT => Some(">>>"),
            TokenKind::PLUSEQ => Some("+="),
            TokenKind::SUBEQ => Some("-="),
            TokenKind::STAREQ => Some("*="),
            TokenKind::SLASHEQ => Some("/="),
            TokenKind::AMPEQ => Some("&="),
            TokenKind::BAREQ => Some("|="),
            TokenKind::CARETEQ => Some("^="),
            TokenKind::PERCENTEQ => Some("%="),
            TokenKind::LTLTEQ => Some("<<="),
            TokenKind::GTGTEQ => Some(">>="),
            TokenKind::GTGTGTEQ => Some(">>>="),
            TokenKind::MONKEYS_AT => Some("@"),
            _ => None,
        }
    }

    /// The token tag (Java: TokenKind.tag).
    pub fn tag(&self) -> TokenTag {
        match self {
            TokenKind::IDENTIFIER => TokenTag::NAMED,
            TokenKind::ASSERT => TokenTag::NAMED,
            TokenKind::BOOLEAN => TokenTag::NAMED,
            TokenKind::BYTE => TokenTag::NAMED,
            TokenKind::CHAR => TokenTag::NAMED,
            TokenKind::DOUBLE => TokenTag::NAMED,
            TokenKind::ENUM => TokenTag::NAMED,
            TokenKind::FLOAT => TokenTag::NAMED,
            TokenKind::INT => TokenTag::NAMED,
            TokenKind::LONG => TokenTag::NAMED,
            TokenKind::SHORT => TokenTag::NAMED,
            TokenKind::SUPER => TokenTag::NAMED,
            TokenKind::THIS => TokenTag::NAMED,
            TokenKind::VOID => TokenTag::NAMED,
            TokenKind::INTLITERAL => TokenTag::NUMERIC,
            TokenKind::LONGLITERAL => TokenTag::NUMERIC,
            TokenKind::FLOATLITERAL => TokenTag::NUMERIC,
            TokenKind::DOUBLELITERAL => TokenTag::NUMERIC,
            TokenKind::CHARLITERAL => TokenTag::NUMERIC,
            TokenKind::STRINGLITERAL => TokenTag::STRING,
            TokenKind::TRUE => TokenTag::NAMED,
            TokenKind::FALSE => TokenTag::NAMED,
            TokenKind::NULL => TokenTag::NAMED,
            TokenKind::UNDERSCORE => TokenTag::NAMED,
            _ => TokenTag::DEFAULT,
        }
    }

    /// Java: TokenKind.toString().
    pub fn to_string_kind(&self) -> Option<String> {
        match self {
            TokenKind::IDENTIFIER => Some("token.identifier".to_string()),
            TokenKind::CHARLITERAL => Some("token.character".to_string()),
            TokenKind::STRINGLITERAL => Some("token.string".to_string()),
            TokenKind::INTLITERAL => Some("token.integer".to_string()),
            TokenKind::LONGLITERAL => Some("token.long-integer".to_string()),
            TokenKind::FLOATLITERAL => Some("token.float".to_string()),
            TokenKind::DOUBLELITERAL => Some("token.double".to_string()),
            TokenKind::ERROR => Some("token.bad-symbol".to_string()),
            TokenKind::EOF => Some("token.end-of-input".to_string()),
            TokenKind::DOT
            | TokenKind::COMMA
            | TokenKind::SEMI
            | TokenKind::LPAREN
            | TokenKind::RPAREN
            | TokenKind::LBRACKET
            | TokenKind::RBRACKET
            | TokenKind::LBRACE
            | TokenKind::RBRACE => Some(format!("'{}'", self.name().unwrap_or_default())),
            _ => self.name().map(str::to_string),
        }
    }

    /// Java: TokenKind.test(TokenKind) (the Predicate<TokenKind> method).
    pub fn test(&self, that: TokenKind) -> bool {
        *self == that
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_string_kind() {
            Some(text) => f.write_str(&text),
            None => f.write_str("CUSTOM"),
        }
    }
}

/// Java: Tokens.Comment.CommentStyle
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentStyle {
    LINE,
    BLOCK,
    JAVADOC,
}

/// Java: Tokens.Comment
pub trait Comment {
    /// Java: String getText() (Java null -> None)
    fn get_text(&self) -> Option<JavaString>;

    /// Java: int getSourcePos(int index)
    fn get_source_pos(&self, index: i32) -> i32;

    /// Java: CommentStyle getStyle()
    fn get_style(&self) -> CommentStyle;

    /// Java: boolean isDeprecated()
    fn is_deprecated(&self) -> bool;
}

/// The value carried by a token (Java: the NamedToken/StringToken/
/// NumericToken subclasses).
#[derive(Clone, Debug)]
pub enum TokenValue {
    /// Java: Token (base class; no name/string value).
    None,
    /// Java: NamedToken.name
    Name(Name),
    /// Java: StringToken.stringVal
    String(JavaString),
    /// Java: NumericToken.stringVal + radix
    Numeric { string_val: JavaString, radix: i32 },
}

/// This is the class representing a javac token. Each token has several fields
/// that are set by the javac lexer (i.e. start/end position, string value, etc).
#[derive(Clone)]
pub struct Token {
    /// The token kind
    pub kind: TokenKind,
    /// The start position of this token
    pub pos: i32,
    /// The end position of this token
    pub end_pos: i32,
    /// Comment reader associated with this token
    pub comments: Option<List<Rc<dyn Comment>>>,
    /// The Java subclass payload (TokenValue::None for the base Token).
    value: TokenValue,
}

impl Token {
    /// Java: Token(TokenKind kind, int pos, int endPos, List<Comment> comments)
    pub fn new(
        kind: TokenKind,
        pos: i32,
        end_pos: i32,
        comments: Option<List<Rc<dyn Comment>>>,
    ) -> Token {
        let token = Token { kind, pos, end_pos, comments, value: TokenValue::None };
        token.check_kind();
        token
    }

    /// Java: NamedToken(...)
    pub fn new_named(
        kind: TokenKind,
        pos: i32,
        end_pos: i32,
        name: Name,
        comments: Option<List<Rc<dyn Comment>>>,
    ) -> Token {
        let token = Token { kind, pos, end_pos, comments, value: TokenValue::Name(name) };
        token.check_kind();
        token
    }

    /// Java: StringToken(...)
    pub fn new_string(
        kind: TokenKind,
        pos: i32,
        end_pos: i32,
        string_val: JavaString,
        comments: Option<List<Rc<dyn Comment>>>,
    ) -> Token {
        let token = Token { kind, pos, end_pos, comments, value: TokenValue::String(string_val) };
        token.check_kind();
        token
    }

    /// Java: NumericToken(...)
    pub fn new_numeric(
        kind: TokenKind,
        pos: i32,
        end_pos: i32,
        string_val: JavaString,
        radix: i32,
        comments: Option<List<Rc<dyn Comment>>>,
    ) -> Token {
        let token = Token {
            kind,
            pos,
            end_pos,
            comments,
            value: TokenValue::Numeric { string_val, radix },
        };
        token.check_kind();
        token
    }

    fn check_kind(&self) {
        let expected = match &self.value {
            TokenValue::None => TokenTag::DEFAULT,
            TokenValue::Name(_) => TokenTag::NAMED,
            TokenValue::String(_) => TokenTag::STRING,
            TokenValue::Numeric { .. } => TokenTag::NUMERIC,
        };
        // Java: if (kind.tag != expected) throw new AssertionError("Bad token kind - expected " + expected);
        if self.kind.tag() != expected {
            panic!("java.lang.AssertionError: Bad token kind - expected {:?}", expected);
        }
    }

    /// Java: Token.split(Tokens) — returns the two subtokens (the first keeps
    /// the comments).
    pub fn split(&self, tokens: &Tokens) -> [Token; 2] {
        let name = match self.kind.name() {
            Some(name) if name.len() >= 2 && self.kind.tag() == TokenTag::DEFAULT => name,
            _ => panic!("java.lang.AssertionError: Cant split {:?}", self.kind),
        };
        let first = tokens.lookup_kind_str(&name[..1]);
        let second = tokens.lookup_kind_str(&name[1..]);
        [
            Token::new(first, self.pos, self.pos + first.name().unwrap_or("").len() as i32, None),
            Token::new(second, self.pos + first.name().unwrap_or("").len() as i32, self.end_pos, None),
        ]
    }

    /// Java: public Name name()
    pub fn name(&self) -> &Name {
        match &self.value {
            TokenValue::Name(name) => name,
            _ => panic!("java.lang.UnsupportedOperationException"),
        }
    }

    /// Java: public String stringVal()
    pub fn string_val(&self) -> &JavaString {
        match &self.value {
            TokenValue::String(value) | TokenValue::Numeric { string_val: value, .. } => value,
            TokenValue::None | TokenValue::Name(_) => {
                panic!("java.lang.UnsupportedOperationException")
            }
        }
    }

    /// Java: public int radix()
    pub fn radix(&self) -> i32 {
        match &self.value {
            TokenValue::Numeric { radix, .. } => *radix,
            _ => panic!("java.lang.UnsupportedOperationException"),
        }
    }

    /// Java: public Comment comment(CommentStyle style) — the first matching
    /// comment (Java: getComments(JAVADOC).head).
    pub fn comment(&self, style: CommentStyle) -> Option<Rc<dyn Comment>> {
        self.get_comments(style).to_vec().into_iter().next()
    }

    /// Java: public boolean deprecatedFlag()
    pub fn deprecated_flag(&self) -> bool {
        self.get_comments(CommentStyle::JAVADOC)
            .iter()
            .any(|comment| comment.is_deprecated())
    }

    /// Java: private List<Comment> getComments(CommentStyle style)
    pub fn get_comments(&self, style: CommentStyle) -> List<Rc<dyn Comment>> {
        match &self.comments {
            None => List::nil(),
            Some(comments) => {
                let filtered: Vec<Rc<dyn Comment>> = comments
                    .to_vec()
                    .into_iter()
                    .filter(|comment| comment.get_style() == style)
                    .collect();
                List::from_iterable(filtered)
            }
        }
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Token")
            .field("kind", &self.kind)
            .field("pos", &self.pos)
            .field("end_pos", &self.end_pos)
            .field("value", &self.value)
            .finish()
    }
}

/// Java: Tokens (keyword table plus the Names instance).
pub struct Tokens {
    names: Rc<Names>,
    keywords: HashMap<String, TokenKind>,
}

impl Tokens {
    /// Java: protected Tokens(Context context) — the Context-based instance is a
    /// registered GAP (Names has no Context registration in the port); the
    /// names parameter keeps the upstream shared Names dependency.
    pub fn new(names: Rc<Names>) -> Tokens {
        let mut keywords = HashMap::new();
        for kind in TokenKind::values() {
            if let Some(name) = kind.name() {
                keywords.insert(name.to_string(), *kind);
            }
        }
        Tokens { names, keywords }
    }

    /// The shared Names instance (Java: Tokens.names).
    pub fn names(&self) -> &Rc<Names> {
        &self.names
    }

    /// Java: TokenKind lookupKind(Name name)
    pub fn lookup_kind(&self, name: &Name) -> TokenKind {
        self.lookup_kind_str(&name.to_string())
    }

    /// Java: TokenKind lookupKind(String name)
    pub fn lookup_kind_str(&self, name: &str) -> TokenKind {
        self.keywords.get(name).copied().unwrap_or(TokenKind::IDENTIFIER)
    }
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tokens").field("keywords", &self.keywords.len()).finish()
    }
}

/// Java: public static final Token DUMMY
pub fn dummy_token() -> Token {
    Token::new(TokenKind::ERROR, 0, 0, None)
}
