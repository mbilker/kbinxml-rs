extern crate proc_macro;

#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, TokenStreamExt};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Brace;
use syn::{parse_macro_input, Expr, Ident, LitStr, Token, Type};

mod kw {
    custom_keyword!(attributes);
    custom_keyword!(default);
    custom_keyword!(include);
    custom_keyword!(inputs);
    custom_keyword!(output);
    custom_keyword!(optional);
    custom_keyword!(transform);
    custom_keyword!(value);
}

#[derive(Debug)]
struct Output {
    struct_name: Ident,
}

#[derive(Debug)]
struct Includes {
    includes: Punctuated<Ident, Token![,]>,
}

#[derive(Debug)]
struct SourceMapping {
    source: LitStr,
    target: Ident,
    target_type: Option<Type>,
}

#[derive(Debug)]
struct Mapping {
    source: LitStr,
    attributes: Option<Punctuated<SourceMapping, Token![,]>>,
    subnodes: Option<Punctuated<Mapping, Token![,]>>,
    value: Option<Ident>,
    transform: Option<Expr>,
    default_value: Option<Expr>,
    optional_value: bool,
}

#[derive(Debug)]
struct InputBlock {
    name: Ident,
    mappings: Punctuated<Mapping, Token![,]>,
}

#[derive(Debug)]
struct Inputs {
    blocks: Punctuated<InputBlock, Token![,]>,
}

#[derive(Debug)]
struct Psmap {
    output: Output,
    includes: Option<Includes>,
    inputs: Inputs,
}

struct PsmapOutput {
    struct_name: Ident,
    definitions: TokenStream2,
    fields: TokenStream2,
}

impl Parse for Output {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<kw::output>()?;
        input.parse::<Token![:]>()?;

        let struct_name = input.parse()?;
        input.parse::<Token![,]>()?;

        Ok(Self { struct_name })
    }
}

impl Parse for Includes {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<kw::include>()?;
        input.parse::<Token![:]>()?;

        let content;
        let _ = bracketed!(content in input);
        let mut includes = content.parse_terminated(Ident::parse)?;

        if !includes.trailing_punct() {
            let span = includes.span();
            includes.push_punct(Token![,]([span]));
        }

        input.parse::<Token![,]>()?;

        Ok(Self { includes })
    }
}

impl ToTokens for Includes {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        self.includes.to_tokens(tokens);
    }
}

impl Parse for SourceMapping {
    fn parse(input: ParseStream) -> Result<Self> {
        let source: LitStr = input.parse()?;
        input.parse::<Token![=>]>()?;
        let target: Ident = input.parse()?;

        let lookahead = input.lookahead1();
        let target_type = if lookahead.peek(Token![as]) {
            input.parse::<Token![as]>()?;

            let target_type: Type = input.parse()?;

            Some(target_type)
        } else {
            None
        };

        Ok(Self {
            source,
            target,
            target_type,
        })
    }
}

impl Mapping {
    fn sub_node_parse(source: LitStr, input: ParseStream) -> Result<Self> {
        let content;
        let _ = braced!(content in input);

        let attributes = if content.parse::<Option<kw::attributes>>()?.is_some() {
            //eprintln!("Mapping: attributes");
            content.parse::<Token![:]>()?;

            let attr_content;
            let _ = braced!(attr_content in content);
            let attributes = attr_content.parse_terminated(SourceMapping::parse)?;

            content.parse::<Token![,]>()?;
            Some(attributes)
        } else {
            None
        };

        let value = if content.parse::<Option<kw::value>>()?.is_some() {
            //eprintln!("Mapping: value");
            content.parse::<Token![=>]>()?;

            let value = content.parse()?;
            content.parse::<Token![,]>()?;

            Some(value)
        } else {
            None
        };

        let transform = if content.parse::<Option<kw::transform>>()?.is_some() {
            //eprintln!("Mapping: transform");
            content.parse::<Token![=>]>()?;

            let value = content.parse()?;
            content.parse::<Token![,]>()?;

            Some(value)
        } else {
            None
        };

        let default_value = if content.parse::<Option<kw::default>>()?.is_some() {
            //eprintln!("Mapping: default");
            content.parse::<Token![=>]>()?;

            let value = content.parse()?;
            content.parse::<Token![,]>()?;

            Some(value)
        } else {
            None
        };

        let optional_value = if content.parse::<Option<kw::optional>>()?.is_some() {
            //eprintln!("Mapping: optional");
            content.parse::<Token![,]>()?;

            true
        } else {
            false
        };

        let subnodes = content.parse_terminated(Mapping::parse)?;

        Ok(Self {
            source,
            attributes,
            subnodes: Some(subnodes),
            value,
            transform,
            default_value,
            optional_value,
        })
    }
}

impl Parse for Mapping {
    fn parse(input: ParseStream) -> Result<Self> {
        let source: LitStr = input.parse()?;
        input.parse::<Token![=>]>()?;

        let lookahead = input.lookahead1();
        if lookahead.peek(Brace) {
            Self::sub_node_parse(source, input)
        } else if lookahead.peek(Ident) {
            Ok(Self {
                source,
                attributes: None,
                subnodes: None,
                value: input.parse()?,
                transform: None,
                default_value: None,
                optional_value: false,
            })
        } else {
            panic!("unknown mapping type found");
        }
    }
}

impl Parse for InputBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;

        let content;
        let _ = braced!(content in input);
        let mappings = content.parse_terminated(Mapping::parse)?;

        Ok(Self { name, mappings })
    }
}

impl Parse for Inputs {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<kw::inputs>()?;
        input.parse::<Token![:]>()?;

        let content;
        let _ = bracketed!(content in input);
        let blocks = content.parse_terminated(InputBlock::parse)?;

        input.parse::<Option<Token![,]>>()?;

        Ok(Self { blocks })
    }
}

impl Parse for Psmap {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut output: Option<Output> = None;
        let mut includes: Option<Includes> = None;
        let mut inputs: Option<Inputs> = None;

        loop {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::output) {
                output = Some(input.parse()?);
            } else if lookahead.peek(kw::include) {
                includes = Some(input.parse()?);
            } else if lookahead.peek(kw::inputs) {
                inputs = Some(input.parse()?);
            } else {
                break;
            }
        }

        let output = output.unwrap();
        let inputs = inputs.unwrap();

        //eprintln!("struct_name: {}", output.struct_name);

        /*
        if let Some(ref includes) = includes {
          for include in includes.includes.iter() {
            eprintln!("include: {:?}", include);
          }
        }

        let remaining: TokenStream2 = input.parse()?;
        eprintln!("remaining: {}", remaining);
        */

        Ok(Self {
            output,
            includes,
            inputs,
        })
    }
}

impl PsmapOutput {
    fn value_for_pair(&mut self, mapping: &Mapping, target: &Ident) -> TokenStream2 {
        let struct_name = &self.struct_name;
        let Mapping {
            source,
            transform,
            default_value,
            optional_value,
            ..
        } = mapping;

        let transform = transform.as_ref().map(|transform| {
            quote_spanned! {transform.span()=>
              let child_value = #transform(child_value)?;
            }
        });
        let map_value = match default_value {
            Some(default_value) => quote_spanned! {source.span()=>
              .unwrap_or_else(#default_value)
            },
            None => quote_spanned! {source.span()=>
              .ok_or(::psmap::PsmapError::ValueNotFound {
                source_name: stringify!(#source),
              })?
            },
        };

        let definition_tokens = quote_spanned! {target.span()=>
          let mut #target = None;
        };
        // This part is a little more confusing, but here's the process.
        //
        // `Node::value` returns `Option<&Value>` and `TryInto::try_into` should only be called
        // if there is `Some(value)`, but this returns `Option<Result<T, E>>`. `Option::transpose`
        // converts that to `Result<Option<T>, E>` which `?` can be used on.
        let body_tokens = quote_spanned! {source.span()=>
          let child_value = child.value()
            .map(|v| v.try_into())
            .transpose()?
            #map_value;
          #transform
          #target = Some(child_value);
        };
        let field_tokens = match default_value {
            Some(default_value) => quote_spanned! {target.span()=>
              #target: #target.unwrap_or_else(#default_value),
            },
            None if *optional_value => quote_spanned! {target.span()=>
              #target,
            },
            None => quote_spanned! {target.span()=>
              #target: #target.ok_or(::psmap::PsmapError::FieldNotFound {
                target: stringify!(#target),
                struct_name: stringify!(#struct_name),
              })?,
            },
        };

        self.definitions.append_all(definition_tokens);
        self.fields.append_all(field_tokens);

        body_tokens
    }

    fn handle_mapping(&mut self, mapping: &Mapping) -> TokenStream2 {
        let Mapping {
            source,
            attributes,
            subnodes,
            value,
            ..
        } = mapping;

        let mut body = TokenStream2::new();

        //eprintln!("source: {}, value: {:?}", source.value(), value);

        if let Some(value_target) = value {
            let body_tokens = self.value_for_pair(mapping, value_target);
            body.append_all(body_tokens);
        }

        if let Some(attributes) = attributes {
            let struct_name = &self.struct_name;

            for SourceMapping {
                source: attr,
                target,
                target_type,
            } in attributes.iter()
            {
                let target_type = target_type.as_ref().map(|target_type| {
                    quote! {
                      ::<#target_type>
                    }
                });

                self.definitions.append_all(quote_spanned! {attr.span()=>
                    let mut #target = None;
                });
                body.append_all(quote_spanned! {attr.span()=>
                    #target = Some(
                        child
                            .attributes()
                            .get(#attr)
                            .ok_or(::psmap::PsmapError::AttributeNotFound {
                                attribute: #attr,
                                source_name: #source,
                                struct_name: stringify!(#struct_name),
                            })?
                            .parse#target_type()
                            .map_err(|source| ::psmap::PsmapError::AttributeParse {
                                attribute: #attr,
                                source_name: #source,
                                struct_name: stringify!(#struct_name),
                                source: Box::new(source),
                            })?
                    );
                });
                self.fields.append_all(quote_spanned! {target.span()=>
                    #target: #target.ok_or(::psmap::PsmapError::FieldNotFoundFromSource {
                        target: stringify!(#target),
                        source_name: #source,
                        struct_name: stringify!(#struct_name),
                    })?,
                });
            }
        }

        let inner_loop: Option<TokenStream2> = if let Some(subnodes) = subnodes {
            let input = Ident::new("child", source.span());

            Some(self.create_input_loop(&input, subnodes.iter()))
        } else {
            None
        };

        quote_spanned! {source.span()=>
            #source => {
                #body
                #inner_loop
            },
        }
    }

    fn create_input_loop<'a, I>(&mut self, input: &Ident, mappings: I) -> TokenStream2
    where
        I: Iterator<Item = &'a Mapping>,
    {
        let mut mapping_tokens = TokenStream2::new();

        for mapping in mappings {
            let matching_arm = self.handle_mapping(mapping);
            mapping_tokens.append_all(matching_arm);
        }

        quote! {
            for child in #input.children() {
                match child.key() {
                    #mapping_tokens
                    _ => {},
                };
            }
        }
    }
}

#[proc_macro]
pub fn psmap(input: TokenStream) -> TokenStream {
    let Psmap {
        output: Output { struct_name },
        includes,
        inputs: Inputs { blocks },
    } = parse_macro_input!(input as Psmap);

    let mut output = PsmapOutput {
        struct_name: struct_name.clone(),
        definitions: TokenStream2::new(),
        fields: TokenStream2::new(),
    };

    let mut loops = TokenStream2::new();

    for InputBlock { name, mappings } in blocks.iter() {
        loops.append_all(output.create_input_loop(name, mappings.iter()));
    }

    let definitions = output.definitions;
    let fields = output.fields;

    let output = quote! {
        {
            use std::convert::TryInto;

            #definitions
            #loops

            #struct_name {
                #includes
                #fields
            }
        }
    };
    //eprintln!("output: {}", output);

    output.into()
}
