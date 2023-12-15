use darling::{
    ast::{Data, Fields},
    util::Ignored,
    FromDeriveInput, FromField, FromMeta, FromVariant,
};
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{
    parse::Parse, parse_macro_input, punctuated::Punctuated, Attribute, Lit, LitStr, NestedMeta,
    Path, Token, Type,
};

#[derive(FromDeriveInput)]
#[darling(forward_attrs, attributes(slashery))]
struct SlashCmd {
    ident: Ident,
    data: Data<Ignored, SlashArg>,
    attrs: Vec<Attribute>,
    name: String,
    kind: Path,
}

#[derive(FromField)]
#[darling(forward_attrs)]
struct SlashArg {
    ident: Option<Ident>,
    ty: Type,
    attrs: Vec<Attribute>,
}

#[derive(FromDeriveInput)]
struct SlashCmds {
    ident: Ident,
    data: Data<SlashCmdsCmd, Ignored>,
}

#[derive(FromVariant)]
struct SlashCmdsCmd {
    ident: Ident,
    fields: Fields<SlashCmdsCmdField>,
}

#[derive(FromField)]
struct SlashCmdsCmdField {
    ty: Type,
}

#[derive(FromDeriveInput)]
struct SlashComponents {
    ident: Ident,
    data: Data<SlashComponentsComponent, Ignored>,
}

#[derive(FromVariant)]
#[darling(attributes(slashery))]
struct SlashComponentsComponent {
    ident: Ident,
    #[darling(default)]
    id_alias: StrList,
}

#[derive(Default)]
struct StrList {
    values: Vec<String>,
}

impl FromMeta for StrList {
    fn from_list(items: &[syn::NestedMeta]) -> darling::Result<Self> {
        Ok(StrList {
            values: items
                .iter()
                .map(|item| match item {
                    NestedMeta::Lit(Lit::Str(s)) => Ok(s.value()),
                    _ => panic!(),
                })
                .collect::<darling::Result<Vec<String>>>()?,
        })
    }
}

struct DocAttrTokens {
    _eq: Token![=],
    content: LitStr,
}

impl Parse for DocAttrTokens {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            _eq: input.parse()?,
            content: input.parse()?,
        })
    }
}

#[proc_macro_derive(SlashCmd, attributes(slashery))]
pub fn derive_slash_cmd(item: TokenStream1) -> TokenStream1 {
    let SlashCmd {
        ident,
        data,
        attrs,
        name,
        kind,
    } = match SlashCmd::from_derive_input(&parse_macro_input!(item)) {
        Ok(x) => x,
        Err(err) => return err.write_errors().into(),
    };
    let description = attrs
        .iter()
        .find(|attr| attr.path.is_ident("doc"))
        .map(|doc| {
            syn::parse2::<DocAttrTokens>(doc.tokens.clone())
                .unwrap()
                .content
                .value()
        })
        .unwrap_or_default();
    let description = description.trim();
    let (arg_metas, arg_parsers) = data
        .take_struct()
        .unwrap()
        .into_iter()
        .map(|SlashArg { ident, ty, attrs }| {
            let ident = ident.unwrap();
            let name = ident.to_string();
            let description = attrs
                .iter()
                .find(|attr| attr.path.is_ident("doc"))
                .map(|doc| {
                    syn::parse2::<DocAttrTokens>(doc.tokens.clone())
                        .unwrap()
                        .content
                        .value()
                })
                .unwrap_or_default();
            let description = description.trim();
            (
                quote! {
                    ::slashery::SlashArgMeta {
                        name: #name.to_string(),
                        description: #description.to_string(),
                        kind: <#ty as ::slashery::SlashArg>::arg_discord_type(),
                        required: <#ty as ::slashery::SlashArg>::arg_required(),
                        choices: <#ty as ::slashery::SlashArg>::arg_choices(),
                    },
                },
                quote! {
                    #ident: <#ty as ::slashery::SlashArg>::arg_parse(args.remove(#name))
                        .map_err(|source| ::slashery::CmdFromInteractionError::Arg { source, name: #name.to_string() })?,
                },
            )
        })
        .unzip::<TokenStream, TokenStream, TokenStream, TokenStream>();
    (quote! {
        impl ::slashery::SlashCmd for #ident {
            fn name() -> String {
                #name.to_string()
            }

            fn meta() -> ::slashery::SlashCmdMeta {
                ::slashery::SlashCmdMeta {
                    name: #name.to_string(),
                    description: #description.to_string(),
                    kind: #kind,
                    options: <Self as ::slashery::SlashArgs>::args_meta(),
                }
            }
        }

        impl ::slashery::SlashArgs for #ident {
            fn args_meta() -> Vec<::slashery::SlashArgMeta> {
                vec![#arg_metas]
            }

            fn from_interaction(opts: &[::serenity::model::application::interaction::application_command::CommandDataOption]) -> Result<Self, ::slashery::CmdFromInteractionError> {
                let mut args = opts.iter().map(|arg| (arg.name.as_str(), arg)).collect::<::std::collections::HashMap<_, _>>();
                Ok(Self {
                    #arg_parsers
                })
            }
        }
    })
    .into()
}

#[proc_macro_derive(SlashCmds)]
pub fn derive_slash_cmds(item: TokenStream1) -> TokenStream1 {
    let SlashCmds { ident, data } = match SlashCmds::from_derive_input(&parse_macro_input!(item)) {
        Ok(x) => x,
        Err(err) => return err.write_errors().into(),
    };
    let data = data.take_enum().unwrap();
    let cmd_metas = data
        .iter()
        .flat_map(|SlashCmdsCmd { fields, .. }| &fields.fields)
        .map(|SlashCmdsCmdField { ty }| {
            quote! { <#ty as ::slashery::SlashCmd>::meta(), }
        })
        .collect::<TokenStream>();
    let cmd_from_interactions = data
        .iter()
        .map(
            |SlashCmdsCmd {
                 fields,
                 ident: field_ident,
             }| {
                let field = &fields.fields[0];
                let ty = &field.ty;
                quote! {
                    if interaction.data.name == <#ty as ::slashery::SlashCmd>::name() {
                        #ty::from_interaction(&interaction.data.options)
                            .map(#ident::#field_ident)
                            .map_err(|source| ::slashery::CmdsFromInteractionError::Cmd { source, name: <#ty as ::slashery::SlashCmd>::name() })
                    } else
                }
            },
        )
        .collect::<TokenStream>();
    (quote! {
        impl ::slashery::SlashCmds for #ident {
            fn meta() -> Vec<::slashery::SlashCmdMeta> {
                vec![#cmd_metas]
            }

            fn from_interaction(
                interaction: &ApplicationCommandInteraction,
            ) -> Result<Self, ::slashery::CmdsFromInteractionError> {
                #cmd_from_interactions {
                    Err(::slashery::CmdsFromInteractionError::UnknownCmd { name: interaction.data.name.to_string() })
                }
            }
        }
    })
    .into()
}

#[proc_macro_derive(SlashComponents, attributes(slashery))]
pub fn derive_slash_components(item: TokenStream1) -> TokenStream1 {
    let SlashComponents { ident, data } =
        match SlashComponents::from_derive_input(&parse_macro_input!(item)) {
            Ok(x) => x,
            Err(err) => return err.write_errors().into(),
        };
    let data = data.take_enum().unwrap();
    let components_from_interactions = data
        .iter()
        .map(
            |SlashComponentsComponent {
                 ident: variant_ident,
                 id_alias,
             }| {
                let id_match = [variant_ident.to_string()]
                    .iter() //std::iter::once(variant_ident.to_string())
                    .chain(id_alias.values.iter()) //.map(LitStr::value))
                    .map(|id| quote! { interaction.data.custom_id == #id })
                    .collect::<Punctuated<TokenStream, Token![||]>>();
                quote! {
                    if #id_match {
                        Ok(Self::#variant_ident)
                    } else
                }
            },
        )
        .collect::<TokenStream>();
    let component_id_match = data
        .iter()
        .map(
            |SlashComponentsComponent {
                 ident: variant_ident,
                 ..
             }| {
                let id = variant_ident.to_string();
                quote! { Self::#variant_ident => #id, }
            },
        )
        .collect::<TokenStream>();
    (quote! {
        impl ::slashery::SlashComponents for #ident {
            fn from_interaction(
                interaction: &MessageComponentInteraction,
            ) -> Result<Self, ::slashery::ComponentsFromInteractionError> {
                #components_from_interactions {
                    Err(::slashery::ComponentsFromInteractionError::UnknownComponent {
                        id: interaction.data.custom_id.to_string(),
                    })
                }
            }

            fn component_id(&self) -> &'static str {
                match self {
                    #component_id_match
                }
            }
        }
    })
    .into()
}
