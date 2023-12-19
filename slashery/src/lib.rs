use serde::Serialize;
use serde_repr::Serialize_repr;
use serenity::model::application::{
    command::{CommandOptionChoice, CommandOptionType as ApplicationCommandOptionType},
    interaction::{
        application_command::{
            ApplicationCommandInteraction,
            CommandDataOption as ApplicationCommandInteractionDataOption,
        },
        message_component::MessageComponentInteraction,
    },
};
pub use slashery_derive::{SlashCmd, SlashCmds, SlashComponents};
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum CmdsFromInteractionError {
    #[snafu(display("failed to parse command {name:?}"))]
    Cmd {
        source: CmdFromInteractionError,
        name: String,
    },
    #[snafu(display("unknown command {name:?}"))]
    UnknownCmd { name: String },
}

#[derive(Debug, Snafu)]
pub enum CmdFromInteractionError {
    #[snafu(display("failed to parse argument {name:?}"))]
    Arg {
        source: ArgFromInteractionError,
        name: String,
    },
}

#[derive(Debug, Snafu)]
pub enum ArgFromInteractionError {
    #[snafu(display("invalid type: expected {expected:?} but got {got:?}"))]
    InvalidType {
        expected: ApplicationCommandOptionType,
        got: ApplicationCommandOptionType,
    },
    #[snafu(display(
        "invalid value: got value {got:?}: {message:?}",
        message = message.clone().unwrap_or_else(|| format!("expected value of type {expected:?}"))
    ))]
    InvalidValueForType {
        expected: ApplicationCommandOptionType,
        got: serde_json::Value,
        message: Option<String>,
    },
    #[snafu(display("required field not found"))]
    FieldNotFound,
}

#[derive(Debug, Snafu)]
pub enum ComponentsFromInteractionError {
    UnknownComponent { id: String },
}

pub trait SlashCmds: Sized {
    fn meta() -> Vec<SlashCmdMeta>;
    fn from_interaction(
        interaction: &ApplicationCommandInteraction,
    ) -> Result<Self, CmdsFromInteractionError>;
}

pub trait SlashCmd {
    fn name() -> String;
    fn meta() -> SlashCmdMeta;
}

pub trait SlashArgs: Sized {
    fn args_meta() -> Vec<SlashArgMeta>;
    fn from_interaction(
        opts: &[ApplicationCommandInteractionDataOption],
    ) -> Result<Self, CmdFromInteractionError>;
}

pub trait SlashArg: Sized {
    fn arg_parse(
        arg: Option<&ApplicationCommandInteractionDataOption>,
    ) -> Result<Self, ArgFromInteractionError>;
    fn arg_discord_type() -> ApplicationCommandOptionType;
    fn arg_required() -> bool;
    fn arg_choices() -> Vec<CommandOptionChoice> {
        Vec::new()
    }
}

impl SlashArg for String {
    fn arg_parse(
        arg: Option<&ApplicationCommandInteractionDataOption>,
    ) -> Result<Self, ArgFromInteractionError> {
        if let Some(arg) = arg {
            if arg.kind == ApplicationCommandOptionType::String {
                let value = arg
                    .value
                    .clone()
                    .ok_or(ArgFromInteractionError::FieldNotFound)?;
                Ok(value.as_str().map(|v| v.to_string()).ok_or(
                    ArgFromInteractionError::InvalidValueForType {
                        expected: ApplicationCommandOptionType::String,
                        got: value,
                        message: None,
                    },
                )?)
            } else {
                Err(ArgFromInteractionError::InvalidType {
                    expected: ApplicationCommandOptionType::String,
                    got: arg.kind,
                })
            }
        } else {
            Err(ArgFromInteractionError::FieldNotFound)
        }
    }

    fn arg_discord_type() -> ApplicationCommandOptionType {
        ApplicationCommandOptionType::String
    }

    fn arg_required() -> bool {
        true
    }
}

impl<T: SlashArg> SlashArg for Option<T> {
    fn arg_parse(
        arg: Option<&ApplicationCommandInteractionDataOption>,
    ) -> Result<Self, ArgFromInteractionError> {
        if let Some(arg) = arg {
            T::arg_parse(Some(arg)).map(Some)
        } else {
            Ok(None)
        }
    }

    fn arg_discord_type() -> ApplicationCommandOptionType {
        T::arg_discord_type()
    }

    fn arg_required() -> bool {
        false
    }
}

#[derive(Serialize, Debug)]
pub struct SlashCmdMeta {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: SlashCmdType,
    pub options: Vec<SlashArgMeta>,
}

#[repr(u8)]
#[derive(Serialize_repr, Debug)]
pub enum SlashCmdType {
    /// Slash command
    ChatInput = 1,
}

#[derive(Serialize, Debug)]
pub struct SlashArgMeta {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: ApplicationCommandOptionType,
    pub required: bool,
    pub choices: Vec<CommandOptionChoice>,
}

pub trait SlashComponents: Sized {
    fn from_interaction(
        interaction: &MessageComponentInteraction,
    ) -> Result<Self, ComponentsFromInteractionError>;

    fn component_id(&self) -> &'static str;
}
