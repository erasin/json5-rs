use pest::iterators::{Pair, Pairs};
use pest::Parser;
use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, SeqAccess, Visitor,
};
use std::char;
use std::f64::{INFINITY, NAN, NEG_INFINITY};

use error::{Error, Result};

const _GRAMMAR: &str = include_str!("json5.pest");

#[derive(Parser)]
#[grammar = "json5.pest"]
struct JSON5Parser;

pub struct Json5Deserializer<'de> {
    pair: Option<Pair<'de, Rule>>,
}

impl<'de> Json5Deserializer<'de> {
    pub fn from_str(input: &'de str) -> Result<Self> {
        let pair = JSON5Parser::parse(Rule::text, input)?.next().unwrap();
        Ok(Json5Deserializer::from_pair(pair))
    }

    fn from_pair(pair: Pair<'de, Rule>) -> Self {
        Json5Deserializer { pair: Some(pair) }
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Json5Deserializer::from_str(s)?;
    T::deserialize(&mut deserializer)
}

impl<'de, 'a> Deserializer<'de> for &'a mut Json5Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let pair = self.pair.take().unwrap();
        match pair.as_rule() {
            Rule::null => visitor.visit_unit(),
            Rule::boolean => visitor.visit_bool(parse_bool(pair)),
            Rule::string => visitor.visit_string(parse_string(pair)),
            Rule::number => visitor.visit_f64(parse_number(pair)),
            Rule::array => visitor.visit_seq(Access::to(pair.into_inner())),
            // TODO
            // Rule::object => visitor.visit_map(Access::to(pair.into_inner())),
            _ => unreachable!(),
        }
    }

    // TODO Probably don't want to forward enum, struct, etc...
    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

fn parse_bool(pair: Pair<Rule>) -> bool {
    match pair.as_str() {
        "true" => true,
        "false" => false,
        _ => unreachable!(),
    }
}

fn parse_string(pair: Pair<Rule>) -> String {
    pair.into_inner()
        .map(|component| match component.as_rule() {
            Rule::char_literal => String::from(component.as_str()),
            Rule::char_escape_sequence => parse_char_escape_sequence(component),
            Rule::nul_escape_sequence => String::from("\u{0000}"),
            Rule::hex_escape_sequence | Rule::unicode_escape_sequence => {
                char::from_u32(parse_hex(component.as_str()))
                    .unwrap()
                    .to_string()
            }
            _ => unreachable!(),
        })
        .collect()
}

fn parse_char_escape_sequence(pair: Pair<Rule>) -> String {
    String::from(match pair.as_str() {
        "b" => "\u{0008}",
        "f" => "\u{000C}",
        "n" => "\n",
        "r" => "\r",
        "t" => "\t",
        "v" => "\u{000B}",
        c => c,
    })
}

fn parse_number(pair: Pair<Rule>) -> f64 {
    match pair.as_str() {
        "Infinity" => INFINITY,
        "-Infinity" => NEG_INFINITY,
        "NaN" | "-NaN" => NAN,
        s if is_hex_literal(s) => parse_hex(&s[2..]) as f64,
        s => s.parse().unwrap(),
    }
}

fn parse_hex(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}

fn is_hex_literal(s: &str) -> bool {
    s.len() > 2 && (&s[..2] == "0x" || &s[..2] == "0X")
}

struct Access<'de> {
    pairs: Pairs<'de, Rule>,
}

impl<'de> Access<'de> {
    fn to(pairs: Pairs<'de, Rule>) -> Self {
        Access { pairs }
    }
}

impl<'de> SeqAccess<'de> for Access<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if let Some(pair) = self.pairs.next() {
            seed.deserialize(&mut Json5Deserializer::from_pair(pair))
                .map(Some)
        } else {
            Ok(None)
        }
    }
}
