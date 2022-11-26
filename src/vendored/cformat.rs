//! Implementation of Printf-Style string formatting
//! as per the [Python Docs](https://docs.python.org/3/library/stdtypes.html#printf-style-string-formatting).
//! Vendored from [cformat.rs in rustpython-vm](https://github.com/RustPython/RustPython/blob/f54b5556e28256763c5506813ea977c9e1445af0/vm/src/cformat.rs).
//! The only changes we make are to remove dead code and code involving the vm.
use std::fmt;
use std::iter::{Enumerate, Peekable};
use std::str::FromStr;

use bitflags::bitflags;

#[derive(Debug, PartialEq)]
pub(crate) enum CFormatErrorType {
    UnmatchedKeyParentheses,
    MissingModuloSign,
    UnsupportedFormatChar(char),
    IncompleteFormat,
    IntTooBig,
    // Unimplemented,
}

// also contains how many chars the parsing function consumed
type ParsingError = (CFormatErrorType, usize);

#[derive(Debug, PartialEq)]
pub(crate) struct CFormatError {
    pub(crate) typ: CFormatErrorType,
    index: usize,
}

impl fmt::Display for CFormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use CFormatErrorType::*;
        match self.typ {
            UnmatchedKeyParentheses => write!(f, "incomplete format key"),
            CFormatErrorType::IncompleteFormat => write!(f, "incomplete format"),
            UnsupportedFormatChar(c) => write!(
                f,
                "unsupported format character '{}' ({:#x}) at index {}",
                c, c as u32, self.index
            ),
            IntTooBig => write!(f, "width/precision too big"),
            _ => write!(f, "unexpected error parsing format string"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum CFormatPreconversor {
    Repr,
    Str,
    Ascii,
    Bytes,
}

#[derive(Debug, PartialEq)]
enum CFormatCase {
    Lowercase,
    Uppercase,
}

#[derive(Debug, PartialEq)]
enum CNumberType {
    Decimal,
    Octal,
    Hex(CFormatCase),
}

#[derive(Debug, PartialEq)]
enum CFloatType {
    Exponent(CFormatCase),
    PointDecimal(CFormatCase),
    General(CFormatCase),
}

#[derive(Debug, PartialEq)]
enum CFormatType {
    Number(CNumberType),
    Float(CFloatType),
    Character,
    String(CFormatPreconversor),
}

bitflags! {
    struct CConversionFlags: u32 {
        const ALTERNATE_FORM = 0b0000_0001;
        const ZERO_PAD = 0b0000_0010;
        const LEFT_ADJUST = 0b0000_0100;
        const BLANK_SIGN = 0b0000_1000;
        const SIGN_CHAR = 0b0001_0000;
    }
}

#[derive(Debug, PartialEq)]
enum CFormatQuantity {
    Amount(usize),
    FromValuesTuple,
}

#[derive(Debug, PartialEq)]
struct CFormatSpec {
    mapping_key: Option<String>,
    flags: CConversionFlags,
    min_field_width: Option<CFormatQuantity>,
    precision: Option<CFormatQuantity>,
    format_type: CFormatType,
    format_char: char,
    // chars_consumed: usize,
}

impl CFormatSpec {
    fn parse<T, I>(iter: &mut ParseIter<I>) -> Result<Self, ParsingError>
    where
        T: Into<char> + Copy,
        I: Iterator<Item = T>,
    {
        let mapping_key = parse_spec_mapping_key(iter)?;
        let flags = parse_flags(iter);
        let min_field_width = parse_quantity(iter)?;
        let precision = parse_precision(iter)?;
        consume_length(iter);
        let (format_type, format_char) = parse_format_type(iter)?;
        let precision = precision.or(match format_type {
            CFormatType::Float(_) => Some(CFormatQuantity::Amount(6)),
            _ => None,
        });

        Ok(CFormatSpec {
            mapping_key,
            flags,
            min_field_width,
            precision,
            format_type,
            format_char,
        })
    }
}

#[derive(Debug, PartialEq)]
enum CFormatPart<T> {
    Literal(T),
    Spec(CFormatSpec),
}

impl<T> CFormatPart<T> {
    fn is_specifier(&self) -> bool {
        matches!(self, CFormatPart::Spec(_))
    }

    fn has_key(&self) -> bool {
        match self {
            CFormatPart::Spec(s) => s.mapping_key.is_some(),
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct CFormatString {
    parts: Vec<(usize, CFormatPart<String>)>,
}

impl FromStr for CFormatString {
    type Err = CFormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut iter = text.chars().enumerate().peekable();
        Self::parse(&mut iter)
    }
}

impl CFormatString {
    pub(crate) fn parse<I: Iterator<Item = char>>(
        iter: &mut ParseIter<I>,
    ) -> Result<Self, CFormatError> {
        let mut parts = vec![];
        let mut literal = String::new();
        let mut part_index = 0;
        while let Some((index, c)) = iter.next() {
            if c == '%' {
                if let Some(&(_, second)) = iter.peek() {
                    if second == '%' {
                        iter.next().unwrap();
                        literal.push('%');
                        continue;
                    } else {
                        if !literal.is_empty() {
                            parts.push((
                                part_index,
                                CFormatPart::Literal(std::mem::take(&mut literal)),
                            ));
                        }
                        let spec = CFormatSpec::parse(iter).map_err(|err| CFormatError {
                            typ: err.0,
                            index: err.1,
                        })?;
                        parts.push((index, CFormatPart::Spec(spec)));
                        if let Some(&(index, _)) = iter.peek() {
                            part_index = index;
                        }
                    }
                } else {
                    return Err(CFormatError {
                        typ: CFormatErrorType::IncompleteFormat,
                        index: index + 1,
                    });
                }
            } else {
                literal.push(c);
            }
        }
        if !literal.is_empty() {
            parts.push((part_index, CFormatPart::Literal(literal)));
        }
        Ok(Self { parts })
    }
}

type ParseIter<I> = Peekable<Enumerate<I>>;

fn parse_quantity<T, I>(iter: &mut ParseIter<I>) -> Result<Option<CFormatQuantity>, ParsingError>
where
    T: Into<char> + Copy,
    I: Iterator<Item = T>,
{
    if let Some(&(_, c)) = iter.peek() {
        let c: char = c.into();
        if c == '*' {
            iter.next().unwrap();
            return Ok(Some(CFormatQuantity::FromValuesTuple));
        }
        if let Some(i) = c.to_digit(10) {
            let mut num = i as i32;
            iter.next().unwrap();
            while let Some(&(index, c)) = iter.peek() {
                if let Some(i) = c.into().to_digit(10) {
                    num = num
                        .checked_mul(10)
                        .and_then(|num| num.checked_add(i as i32))
                        .ok_or((CFormatErrorType::IntTooBig, index))?;
                    iter.next().unwrap();
                } else {
                    break;
                }
            }
            return Ok(Some(CFormatQuantity::Amount(num.unsigned_abs() as usize)));
        }
    }
    Ok(None)
}

fn parse_precision<T, I>(iter: &mut ParseIter<I>) -> Result<Option<CFormatQuantity>, ParsingError>
where
    T: Into<char> + Copy,
    I: Iterator<Item = T>,
{
    if let Some(&(_, c)) = iter.peek() {
        if c.into() == '.' {
            iter.next().unwrap();
            return parse_quantity(iter);
        }
    }
    Ok(None)
}

fn parse_text_inside_parentheses<T, I>(iter: &mut ParseIter<I>) -> Option<String>
where
    T: Into<char>,
    I: Iterator<Item = T>,
{
    let mut counter: i32 = 1;
    let mut contained_text = String::new();
    loop {
        let (_, c) = iter.next()?;
        let c = c.into();
        match c {
            _ if c == '(' => {
                counter += 1;
            }
            _ if c == ')' => {
                counter -= 1;
            }
            _ => (),
        }

        if counter > 0 {
            contained_text.push(c);
        } else {
            break;
        }
    }

    Some(contained_text)
}

fn parse_spec_mapping_key<T, I>(iter: &mut ParseIter<I>) -> Result<Option<String>, ParsingError>
where
    T: Into<char> + Copy,
    I: Iterator<Item = T>,
{
    if let Some(&(index, c)) = iter.peek() {
        if c.into() == '(' {
            iter.next().unwrap();
            return match parse_text_inside_parentheses(iter) {
                Some(key) => Ok(Some(key)),
                None => Err((CFormatErrorType::UnmatchedKeyParentheses, index)),
            };
        }
    }
    Ok(None)
}

fn parse_flags<T, I>(iter: &mut ParseIter<I>) -> CConversionFlags
where
    T: Into<char> + Copy,
    I: Iterator<Item = T>,
{
    let mut flags = CConversionFlags::empty();
    while let Some(&(_, c)) = iter.peek() {
        let flag = match c.into() {
            '#' => CConversionFlags::ALTERNATE_FORM,
            '0' => CConversionFlags::ZERO_PAD,
            '-' => CConversionFlags::LEFT_ADJUST,
            ' ' => CConversionFlags::BLANK_SIGN,
            '+' => CConversionFlags::SIGN_CHAR,
            _ => break,
        };
        iter.next().unwrap();
        flags |= flag;
    }
    flags
}

fn consume_length<T, I>(iter: &mut ParseIter<I>)
where
    T: Into<char> + Copy,
    I: Iterator<Item = T>,
{
    if let Some(&(_, c)) = iter.peek() {
        let c = c.into();
        if c == 'h' || c == 'l' || c == 'L' {
            iter.next().unwrap();
        }
    }
}

fn parse_format_type<T, I>(iter: &mut ParseIter<I>) -> Result<(CFormatType, char), ParsingError>
where
    T: Into<char>,
    I: Iterator<Item = T>,
{
    use CFloatType::*;
    use CFormatCase::{Lowercase, Uppercase};
    use CNumberType::*;
    let (index, c) = match iter.next() {
        Some((index, c)) => (index, c.into()),
        None => {
            return Err((
                CFormatErrorType::IncompleteFormat,
                iter.peek().map(|x| x.0).unwrap_or(0),
            ));
        }
    };
    let format_type = match c {
        'd' | 'i' | 'u' => CFormatType::Number(Decimal),
        'o' => CFormatType::Number(Octal),
        'x' => CFormatType::Number(Hex(Lowercase)),
        'X' => CFormatType::Number(Hex(Uppercase)),
        'e' => CFormatType::Float(Exponent(Lowercase)),
        'E' => CFormatType::Float(Exponent(Uppercase)),
        'f' => CFormatType::Float(PointDecimal(Lowercase)),
        'F' => CFormatType::Float(PointDecimal(Uppercase)),
        'g' => CFormatType::Float(General(Lowercase)),
        'G' => CFormatType::Float(General(Uppercase)),
        'c' => CFormatType::Character,
        'r' => CFormatType::String(CFormatPreconversor::Repr),
        's' => CFormatType::String(CFormatPreconversor::Str),
        'b' => CFormatType::String(CFormatPreconversor::Bytes),
        'a' => CFormatType::String(CFormatPreconversor::Ascii),
        _ => return Err((CFormatErrorType::UnsupportedFormatChar(c), index)),
    };
    Ok((format_type, c))
}

impl FromStr for CFormatSpec {
    type Err = ParsingError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut chars = text.chars().enumerate().peekable();
        if chars.next().map(|x| x.1) != Some('%') {
            return Err((CFormatErrorType::MissingModuloSign, 1));
        }

        CFormatSpec::parse(&mut chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key() {
        let expected = Ok(CFormatSpec {
            mapping_key: Some("amount".to_owned()),
            format_type: CFormatType::Number(CNumberType::Decimal),
            format_char: 'd',
            min_field_width: None,
            precision: None,
            flags: CConversionFlags::empty(),
        });
        assert_eq!("%(amount)d".parse::<CFormatSpec>(), expected);

        let expected = Ok(CFormatSpec {
            mapping_key: Some("m((u(((l((((ti))))p)))l))e".to_owned()),
            format_type: CFormatType::Number(CNumberType::Decimal),
            format_char: 'd',
            min_field_width: None,
            precision: None,
            flags: CConversionFlags::empty(),
        });
        assert_eq!(
            "%(m((u(((l((((ti))))p)))l))e)d".parse::<CFormatSpec>(),
            expected
        );
    }

    #[test]
    fn test_format_parse_key_fail() {
        assert_eq!(
            "%(aged".parse::<CFormatString>(),
            Err(CFormatError {
                typ: CFormatErrorType::UnmatchedKeyParentheses,
                index: 1
            })
        );
    }

    #[test]
    fn test_format_parse_type_fail() {
        assert_eq!(
            "Hello %n".parse::<CFormatString>(),
            Err(CFormatError {
                typ: CFormatErrorType::UnsupportedFormatChar('n'),
                index: 7
            })
        );
    }

    #[test]
    fn test_incomplete_format_fail() {
        assert_eq!(
            "Hello %".parse::<CFormatString>(),
            Err(CFormatError {
                typ: CFormatErrorType::IncompleteFormat,
                index: 7
            })
        );
    }

    #[test]
    fn test_parse_flags() {
        let expected = Ok(CFormatSpec {
            format_type: CFormatType::Number(CNumberType::Decimal),
            format_char: 'd',
            min_field_width: Some(CFormatQuantity::Amount(10)),
            precision: None,
            mapping_key: None,
            flags: CConversionFlags::all(),
        });
        let parsed = "%  0   -+++###10d".parse::<CFormatSpec>();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_string() {
        assert!("%5.4s".parse::<CFormatSpec>().is_ok());
        assert!("%-5.4s".parse::<CFormatSpec>().is_ok());
    }

    #[test]
    fn test_format_parse() {
        let fmt = "Hello, my name is %s and I'm %d years old";
        let expected = Ok(CFormatString {
            parts: vec![
                (0, CFormatPart::Literal("Hello, my name is ".to_owned())),
                (
                    18,
                    CFormatPart::Spec(CFormatSpec {
                        format_type: CFormatType::String(CFormatPreconversor::Str),
                        format_char: 's',
                        mapping_key: None,
                        min_field_width: None,
                        precision: None,
                        flags: CConversionFlags::empty(),
                    }),
                ),
                (20, CFormatPart::Literal(" and I'm ".to_owned())),
                (
                    29,
                    CFormatPart::Spec(CFormatSpec {
                        format_type: CFormatType::Number(CNumberType::Decimal),
                        format_char: 'd',
                        mapping_key: None,
                        min_field_width: None,
                        precision: None,
                        flags: CConversionFlags::empty(),
                    }),
                ),
                (31, CFormatPart::Literal(" years old".to_owned())),
            ],
        });
        let result = fmt.parse::<CFormatString>();
        assert_eq!(
            result, expected,
            "left = {:#?} \n\n\n right = {:#?}",
            result, expected
        );
    }
}