use proc_macro2::{TokenStream, TokenTree};
use prop::collection::VecStrategy;
use proptest::prelude::*;
use serde::{Deserialize, Serialize};
use syn::{FnArg, ItemFn};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum ObjectType {
    String,
    Float,
    Cons,
    Symbol,
    Integer,
    PosInteger,
    Boolean,
    Unknown,
    Function,
    UnibyteString,
    Vector,
    HashTable,
    Record,
    ByteFn,
    Subr,
    Buffer,
    Nil,
    Char,
    CustomString(String),
    CustomList(Vec<ObjectType>),
}

impl ObjectType {
    const SYMBOL_CHARS: &'static str = "[a-zA-Z][a-zA-Z0-9-]*";
    const MAX_FIXNUM: i64 = i64::MAX >> 8;
    const MIN_FIXNUM: i64 = i64::MIN >> 8;
    // New function to create a strategy for a specific type
    fn strategy(self) -> BoxedStrategy<ArbitraryObjectType> {
        match self {
            ObjectType::String => any::<String>().prop_map(ArbitraryObjectType::String).boxed(),
            ObjectType::Float => any::<f64>().prop_map(ArbitraryObjectType::Float).boxed(),
            ObjectType::Cons => Self::cons_strategy().prop_map(ArbitraryObjectType::Cons).boxed(),
            ObjectType::Symbol => Self::SYMBOL_CHARS.prop_map(ArbitraryObjectType::Symbol).boxed(),
            ObjectType::Integer => {
                Self::fixnum_strategy().prop_map(ArbitraryObjectType::Integer).boxed()
            }
            ObjectType::PosInteger => {
                Self::pos_fixnum_strategy().prop_map(ArbitraryObjectType::Integer).boxed()
            }
            ObjectType::Boolean => any::<bool>().prop_map(ArbitraryObjectType::Boolean).boxed(),
            ObjectType::Unknown => Self::any_object_strategy(),
            ObjectType::UnibyteString => {
                "[a-zA-Z0-9 ]*".prop_map(ArbitraryObjectType::UnibyteString).boxed()
            }
            ObjectType::Vector => prop::collection::vec(Self::any_object_strategy(), 0..10)
                .prop_map(ArbitraryObjectType::Vector)
                .boxed(),
            ObjectType::Record => {
                (Self::SYMBOL_CHARS, prop::collection::vec(Self::any_object_strategy(), 0..10))
                    .prop_map(ArbitraryObjectType::Record)
                    .boxed()
            }
            ObjectType::HashTable => todo!("Strategy for HashTable not implemented"),
            ObjectType::ByteFn => any::<u8>().prop_map(ArbitraryObjectType::ByteFn).boxed(),
            ObjectType::Subr => todo!("Strategy for Subr not implemented"),
            ObjectType::Buffer => any::<String>().prop_map(ArbitraryObjectType::Buffer).boxed(),
            ObjectType::Nil => Just(ArbitraryObjectType::Nil).boxed(),
            ObjectType::Char => any::<char>().prop_map(ArbitraryObjectType::Char).boxed(),
            ObjectType::Function => todo!("Strategy for Function not implemented"),
            ObjectType::CustomString(s) => proptest::string::string_regex(&s)
                .expect("Invalid proptest regex")
                .prop_map(ArbitraryObjectType::String)
                .boxed(),
            ObjectType::CustomList(list) => {
                let arb_list: Vec<_> = list.iter().map(|x| x.clone().strategy()).collect();
                (arb_list, Just(false)).prop_map(ArbitraryObjectType::Cons).boxed()
            }
        }
    }

    fn fixnum_strategy() -> BoxedStrategy<i64> {
        any::<i64>()
            .prop_filter("Fixnum", |x| *x >= Self::MIN_FIXNUM && *x <= Self::MAX_FIXNUM)
            .boxed()
    }

    fn pos_fixnum_strategy() -> BoxedStrategy<i64> {
        any::<i64>()
            .prop_filter("Fixnum", |x| *x >= 0 && *x <= Self::MAX_FIXNUM)
            .boxed()
    }

    fn cons_strategy() -> (VecStrategy<BoxedStrategy<ArbitraryObjectType>>, BoxedStrategy<bool>) {
        (
            prop::collection::vec(Self::any_object_strategy(), 0..10),
            prop_oneof![
                1 => Just(true),
                3 => Just(false),
            ]
            .boxed(),
        )
    }

    pub(crate) fn any_object_strategy() -> BoxedStrategy<ArbitraryObjectType> {
        prop_oneof![
            Just(ArbitraryObjectType::Nil),
            any::<bool>().prop_map(ArbitraryObjectType::Boolean),
            Self::fixnum_strategy().prop_map(ArbitraryObjectType::Integer),
            any::<f64>().prop_map(ArbitraryObjectType::Float),
            any::<String>().prop_map(ArbitraryObjectType::String),
            Self::SYMBOL_CHARS.prop_map(ArbitraryObjectType::Symbol),
            "[a-zA-Z0-9 ]*".prop_map(ArbitraryObjectType::UnibyteString),
            any::<char>().prop_map(ArbitraryObjectType::Char),
        ]
        .boxed()
    }
}

// New function to create a combined strategy from multiple types
pub(crate) fn combined_strategy(types: &[ObjectType]) -> BoxedStrategy<ArbitraryObjectType> {
    // Combine all strategies using prop_oneof!
    match types.len() {
        0 => panic!("At least one type must be provided"),
        1 => types[0].clone().strategy(),
        2 => prop_oneof![types[0].clone().strategy(), types[1].clone().strategy()].boxed(),
        3 => prop_oneof![
            types[0].clone().strategy(),
            types[1].clone().strategy(),
            types[2].clone().strategy()
        ]
        .boxed(),
        4 => prop_oneof![
            types[0].clone().strategy(),
            types[1].clone().strategy(),
            types[2].clone().strategy(),
            types[3].clone().strategy()
        ]
        .boxed(),
        5 => prop_oneof![
            types[0].clone().strategy(),
            types[1].clone().strategy(),
            types[2].clone().strategy(),
            types[3].clone().strategy(),
            types[4].clone().strategy()
        ]
        .boxed(),
        n => panic!("Currently supporting up to 5 combined types, got {n}"),
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub(crate) enum ArbitraryObjectType {
    String(String),
    Float(f64),
    Cons((Vec<ArbitraryObjectType>, bool)),
    Symbol(String),
    Integer(i64),
    Boolean(bool),
    Unknown(Box<ArbitraryObjectType>),
    UnibyteString(String),
    Vector(Vec<ArbitraryObjectType>),
    HashTable(Vec<(ArbitraryObjectType, ArbitraryObjectType)>),
    Record((String, Vec<ArbitraryObjectType>)),
    Nil,
    Function(u8),
    ByteFn(u8),
    Char(char),
    Buffer(String),
    Subr(u8),
}

pub(crate) fn print_args(args: &[Option<ArbitraryObjectType>]) -> String {
    args.iter()
        .map(|x| match x {
            Some(x) => format!("{x}"),
            None => "nil".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl std::fmt::Display for ArbitraryObjectType {
    #[expect(clippy::too_many_lines)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::string::ToString;
        match self {
            ArbitraryObjectType::String(s) => {
                write!(f, "\"")?;
                for c in s.chars() {
                    match c {
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        '\r' => write!(f, "\\r")?,
                        '\\' => write!(f, "\\\\")?,
                        '\"' => write!(f, "\\\"")?,
                        c => write!(f, "{c}")?,
                    }
                }
                write!(f, "\"")
            }
            ArbitraryObjectType::Float(n) => {
                write!(f, "{n}")
            }
            ArbitraryObjectType::Cons(list) => {
                let mut cells: Vec<_> = list.0.iter().map(ToString::to_string).collect();
                let len = list.0.len();
                let dot_end = list.1;
                if dot_end && len >= 2 {
                    cells.insert(len - 1, ".".to_owned());
                }
                let string = cells.join(" ");
                write!(f, "'({string})")
            }
            ArbitraryObjectType::Symbol(s) => {
                write!(f, "'{s}")
            }
            ArbitraryObjectType::Integer(n) => {
                write!(f, "{n}")
            }
            ArbitraryObjectType::Boolean(b) => {
                if *b {
                    write!(f, "t")
                } else {
                    write!(f, "nil")
                }
            }
            ArbitraryObjectType::Unknown(obj) => {
                write!(f, "{obj}")
            }
            ArbitraryObjectType::UnibyteString(s) => {
                write!(f, "\"")?;
                for c in s.chars() {
                    match c {
                        '\n' => write!(f, "\\n")?,
                        '\t' => write!(f, "\\t")?,
                        '\r' => write!(f, "\\r")?,
                        '\\' => write!(f, "\\\\")?,
                        '"' => write!(f, "\\\"")?,
                        c => write!(f, "{c}")?,
                    }
                }
                write!(f, "\"")
            }
            ArbitraryObjectType::Nil => {
                write!(f, "nil")
            }
            ArbitraryObjectType::Vector(vec) => {
                let cells: Vec<_> = vec.iter().map(ToString::to_string).collect();
                let string = cells.join(" ");
                write!(f, "[{string}]")
            }
            ArbitraryObjectType::HashTable(vec) => {
                write!(f, "#s(hash-table data (")?;
                for (key, value) in vec {
                    write!(f, "{key} {value} ")?;
                }
                write!(f, "))")
            }
            ArbitraryObjectType::Record((name, members)) => {
                let cells: Vec<_> = members.iter().map(ToString::to_string).collect();
                let string = cells.join(" ");
                write!(f, "(record '{name} {string})")
            }
            ArbitraryObjectType::Function(arity) => {
                write!(f, "(lambda (")?;
                for i in 0..*arity {
                    write!(f, "arg{i} ")?;
                }
                write!(f, ") nil)")
            }
            ArbitraryObjectType::ByteFn(arity) => {
                write!(f, "(lambda (")?;
                for i in 0..*arity {
                    write!(f, "arg{i} ")?;
                }
                write!(f, ") nil)")
            }
            ArbitraryObjectType::Buffer(name) => {
                write!(f, "(generate-new-buffer {name})")
            }
            ArbitraryObjectType::Subr(arity) => {
                write!(f, "(lambda (")?;
                for i in 0..*arity {
                    write!(f, "arg{i} ")?;
                }
                write!(f, ") nil)")
            }
            ArbitraryObjectType::Char(chr) => match chr {
                '\n' => write!(f, "?\\n"),
                '\t' => write!(f, "?\\t"),
                '\r' => write!(f, "?\\r"),
                '\u{0B}' => write!(f, "?\\v"),
                '\u{0C}' => write!(f, "?\\f"),
                '\u{1B}' => write!(f, "?\\e"),
                '\u{7F}' => write!(f, "?\\d"),
                '\u{08}' => write!(f, "?\\b"),
                '\u{07}' => write!(f, "?\\a"),
                '(' | ')' | '[' | ']' | '\\' | '"' => write!(f, "?\\{chr}"),
                chr => write!(f, "?{chr}"),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum Type {
    Object(Vec<ObjectType>),
    Nil,
}

impl Type {
    fn strategy(&self) -> BoxedStrategy<ArbitraryObjectType> {
        match self {
            Type::Object(types) => combined_strategy(types),
            Type::Nil => Just(ArbitraryObjectType::Nil).boxed(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ArgType {
    Required(Type),
    Optional(Type),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) args: Vec<ArgType>,
    pub(crate) ret: Type,
    pub(crate) fallible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    pub(crate) test_count: u32,
    pub(crate) functions: Vec<Function>,
}

#[allow(dead_code)]
impl Function {
    pub(crate) fn strategy(&self) -> Vec<BoxedStrategy<Option<ArbitraryObjectType>>> {
        self.args
            .iter()
            .map(|arg| match arg {
                ArgType::Required(ty) => ty.strategy().prop_map(Some).boxed(),
                ArgType::Optional(ty) => {
                    prop_oneof![1 => Just(None), 3 => ty.strategy().prop_map(Some)].boxed()
                }
            })
            .collect::<Vec<_>>()
    }

    fn process_arg(ty: &syn::Type) -> Result<Type, String> {
        match ty {
            syn::Type::Array(_) => Err("Array not supported".to_string()),
            syn::Type::BareFn(_) => Err("BareFn not supported".to_string()),
            syn::Type::ImplTrait(_) => Err("Impl Trait not supported".to_string()),
            syn::Type::Infer(_) => Err("Infer not supported".to_string()),
            syn::Type::Macro(_) => Err("Macro not supported".to_string()),
            syn::Type::Never(_) => Err("Never not supported".to_string()),
            syn::Type::Paren(_) => Err("Paren not supported".to_string()),
            syn::Type::Path(syn::TypePath { path, .. }) => {
                let segments = &path.segments;
                let last = segments.last().unwrap().ident.to_string();

                match last.as_str() {
                    "StringOrSymbol" => {
                        Ok(Type::Object(vec![ObjectType::String, ObjectType::Symbol]))
                    }
                    "LispTime" => {
                        Ok(Type::Object(vec![ObjectType::Cons]))
                    }
                    "Symbol" => Ok(Type::Object(vec![ObjectType::Symbol])),
                    "Number" | "NumberValue" => {
                        Ok(Type::Object(vec![ObjectType::Integer, ObjectType::Float]))
                    }
                    "Object" => Ok(Type::Object(vec![ObjectType::Unknown])),
                    "usize" | "isize" | "i64" => Ok(Type::Object(vec![ObjectType::Integer])),
                    "str" | "String" | "LispString" => Ok(Type::Object(vec![ObjectType::String])),
                    "bool" => Ok(Type::Object(vec![ObjectType::Boolean])),
                    "f64" => Ok(Type::Object(vec![ObjectType::Float])),
                    "char" => Ok(Type::Object(vec![ObjectType::Char])),
                    "Function" => Ok(Type::Object(vec![ObjectType::Function])),
                    "Cons" | "List" | "Error" => Ok(Type::Object(vec![ObjectType::Cons])),
                    "OptionalFlag" => Ok(Type::Object(vec![
                        ObjectType::Boolean,
                        ObjectType::Nil,
                        ObjectType::Unknown,
                    ])),
                    "ArgSlice" => Ok(Type::Object(vec![ObjectType::Cons, ObjectType::Nil])),
                    "LispVec" | "LispVector" | "Vec" | "RecordBuilder" => {
                        Ok(Type::Object(vec![ObjectType::Vector]))
                    }
                    "LispHashTable" => Ok(Type::Object(vec![ObjectType::HashTable])),
                    "ByteString" => Ok(Type::Object(vec![ObjectType::UnibyteString])),
                    "Record" => Ok(Type::Object(vec![ObjectType::Record])),
                    "ByteFn" => Ok(Type::Object(vec![ObjectType::ByteFn])),
                    "SubrFn" => Ok(Type::Object(vec![ObjectType::Subr])),
                    "LispBuffer" => Ok(Type::Object(vec![ObjectType::Buffer])),
                    "Rto" | "Rt" | "Gc" => {
                        let syn::PathArguments::AngleBracketed(
                            syn::AngleBracketedGenericArguments { args, .. },
                        ) = &segments.last().unwrap().arguments
                        else {
                            return Err("Expected angle bracketed arguments".to_string());
                        };
                        let mut type_argument = None;
                        for arg in args {
                            match arg {
                                syn::GenericArgument::Type(ty) => {
                                    type_argument = Some(ty);
                                }
                                _ => continue,
                            }
                        }

                        let Some(ty) = type_argument else {
                            return Err("Expected type argument".to_string());
                        };

                        Function::process_arg(ty)
                    }
                    "Env" | "Context" => {
                        Err("Environment or Context type not supported".to_string())
                    }
                    _ => Err(format!("Unknown type: {last}")),
                }
            }
            syn::Type::Ptr(_) => Err("Ptr not supported".to_string()),
            syn::Type::Reference(syn::TypeReference { elem, .. })
            | syn::Type::Group(syn::TypeGroup { elem, .. }) => Function::process_arg(elem),
            syn::Type::Slice(syn::TypeSlice { .. }) => {
                Ok(Type::Object(vec![ObjectType::Cons, ObjectType::Nil]))
            }
            syn::Type::TraitObject(_) => Err("TraitObject not supported".to_string()),
            syn::Type::Tuple(_) => Ok(Type::Object(vec![ObjectType::Nil])),
            syn::Type::Verbatim(_) => Err("Verbatim type not supported".to_string()),
            _ => Err("Unknown type".to_string()),
        }
    }

    fn custom_templates(func: &ItemFn) -> Vec<Option<ObjectType>> {
        for attr in &func.attrs {
            if let syn::Meta::List(list) = &attr.meta {
                if list.path.get_ident().unwrap() == "elprop" {
                    let custom_args = Self::parse_stream(list.tokens.clone());
                    return custom_args
                        .into_iter()
                        .map(|x| match x {
                            ObjectType::Nil => None,
                            x => Some(x),
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }

    fn parse_stream(ts: TokenStream) -> Vec<ObjectType> {
        ts.into_iter()
            .filter(|x| !matches!(x, TokenTree::Punct(_)))
            .map(|token| match token {
                TokenTree::Group(group) => {
                    ObjectType::CustomList(Self::parse_stream(group.stream()))
                }
                TokenTree::Ident(ident) => match ident.to_string().as_ref() {
                    "_" => ObjectType::Nil,
                    "usize" | "u64" => ObjectType::PosInteger,
                    "isize" | "i64" => ObjectType::Integer,
                    x => panic!("Unknown type {x}"),
                },
                TokenTree::Literal(literal) => ObjectType::CustomString(
                    syn::parse_str::<syn::LitStr>(&literal.to_string())
                        .expect("Invalid Literal {literal:?}")
                        .value(),
                ),
                TokenTree::Punct(_) => unreachable!("Punct in stream"),
            })
            .collect()
    }

    pub(crate) fn from_item(item: &ItemFn) -> Result<Self, String> {
        let name = item
            .sig
            .ident
            .to_string()
            .chars()
            .map(|c| match c {
                '_' => '-',
                c => c,
            })
            .collect();

        let args = Function::get_args(item);

        let (ret, fallible) = Self::get_output(item)?;
        Ok(Function { name, args, ret, fallible })
    }

    fn get_args(item: &ItemFn) -> Vec<ArgType> {
        let templates = Self::custom_templates(item);

        item.sig
            .inputs
            .iter()
            .map(|x| match x {
                FnArg::Receiver(syn::Receiver { ty, .. })
                | FnArg::Typed(syn::PatType { ty, .. }) => ty,
            })
            .enumerate()
            .filter_map(|(i, arg)| {
                // If a custom template is specified, use it
                if let Some(Some(template)) = templates.get(i) {
                    return Some(ArgType::Required(Type::Object(vec![template.clone()])));
                }
                match arg.as_ref() {
                    syn::Type::Group(syn::TypeGroup { group_token, elem }) => {
                        let syn::token::Group { span } = group_token;

                        let source_text = span
                            .source_text()
                            .ok_or_else(|| "Failed to get source text".to_string())
                            .ok()?;
                        let optional = matches!(source_text.as_str(), "Option");
                        Self::wrap_arg(optional, elem)
                    }
                    syn::Type::Path(syn::TypePath { path, .. }) => {
                        let segments = &path.segments;
                        let last = segments.last().unwrap().ident.to_string();
                        if matches!(last.as_str(), "Result" | "Option") {
                            let syn::PathArguments::AngleBracketed(
                                syn::AngleBracketedGenericArguments { args, .. },
                            ) = &segments.last().unwrap().arguments
                            else {
                                unreachable!("Expected angle bracketed arguments");
                            };
                            let type_argument = args.iter().fold(None, |acc, arg| match arg {
                                syn::GenericArgument::Type(ty) => Some(ty),
                                _ => acc,
                            });
                            Self::wrap_arg(true, type_argument?)
                        } else {
                            Self::wrap_arg(false, arg)
                        }
                    }
                    x => Some(ArgType::Required(Function::process_arg(x).ok()?)),
                }
            })
            .collect()
    }

    fn wrap_arg(optional: bool, ty: &syn::Type) -> Option<ArgType> {
        let arg = Function::process_arg(ty).ok()?;
        match arg {
            Type::Object(ref obj) if obj.contains(&ObjectType::Boolean) => {
                Some(ArgType::Required(arg))
            }
            x => Some(if optional { ArgType::Optional(x) } else { ArgType::Required(x) }),
        }
    }

    fn get_output(item: &ItemFn) -> Result<(Type, bool), String> {
        Ok(match &item.sig.output {
            syn::ReturnType::Default => (Type::Nil, false),
            syn::ReturnType::Type(_, ty) => match ty.as_ref() {
                syn::Type::Group(syn::TypeGroup { group_token, elem }) => {
                    let syn::token::Group { span } = group_token;

                    let source_text = span
                        .source_text()
                        .ok_or_else(|| "Failed to get source text".to_string())?;
                    let fallible = matches!(source_text.as_str(), "Option" | "Result");

                    let ty = Function::process_arg(elem)?;
                    (ty, fallible)
                }
                syn::Type::Path(syn::TypePath { path, .. }) => {
                    let segments = &path.segments;
                    let last = segments.last().unwrap().ident.to_string();
                    match last.as_str() {
                        "Result" | "Option" => {
                            let syn::PathArguments::AngleBracketed(
                                syn::AngleBracketedGenericArguments { args, .. },
                            ) = &segments.last().unwrap().arguments
                            else {
                                unreachable!("Expected angle bracketed arguments");
                            };
                            let mut type_argument = None;
                            for arg in args {
                                match arg {
                                    syn::GenericArgument::Type(ty) => {
                                        type_argument = Some(ty);
                                    }
                                    _ => continue,
                                }
                            }

                            let Some(ty) = type_argument else {
                                return Err("Expected type argument".to_string());
                            };

                            let ty = Function::process_arg(ty)?;
                            (ty, true)
                        }
                        _ => {
                            let ty = Function::process_arg(ty)?;
                            (ty, false)
                        }
                    }
                }
                x => {
                    let ty = Function::process_arg(x)?;
                    (ty, false)
                }
            },
        })
    }
}