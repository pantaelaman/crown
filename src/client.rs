// SPDX-FileCopyrightText: 2022 Harish Rajagopal <harish.rajagopals@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Client that communicates with greetd

use std::env;
use std::io::Result as IOResult;

use greetd_ipc::{
  codec::{Error as GreetdError, TokioCodec},
  Request, Response,
};
use tokio::net::UnixStream;
use tracing::info;

/// Environment variable containing the path to the greetd socket
const GREETD_SOCK_ENV_VAR: &str = "GREETD_SOCK";

pub type GreetdResult = Result<Response, GreetdError>;

/// The authentication status of the current greetd session
#[derive(Clone)]
pub enum AuthStatus {
  NotStarted,
  InProgress,
  Done,
}

/// Client that uses UNIX sockets to communicate with greetd
pub struct GreetdClient {
  /// Socket to communicate with greetd
  socket: UnixStream,
  /// Current authentication status
  auth_status: AuthStatus,
}

impl GreetdClient {
  /// Initialize the socket to communicate with greetd.
  pub async fn new() -> IOResult<Self> {
    let sock_path = env::var(GREETD_SOCK_ENV_VAR).unwrap_or_else(|_| {
      panic!("Missing environment variable '{GREETD_SOCK_ENV_VAR}'. Is greetd running?",)
    });
    let socket = UnixStream::connect(sock_path).await?;
    Ok(Self {
      socket,
      auth_status: AuthStatus::NotStarted,
    })
  }

  /// Initialize a greetd session.
  pub async fn create_session(&mut self, username: &str) -> GreetdResult {
    info!("Creating session for username: {username}");
    let msg = Request::CreateSession {
      username: username.to_string(),
    };
    msg.write_to(&mut self.socket).await?;

    let resp = Response::read_from(&mut self.socket).await?;
    match resp {
      Response::Success => {
        self.auth_status = AuthStatus::Done;
      }
      Response::AuthMessage { .. } => {
        self.auth_status = AuthStatus::InProgress;
      }
      Response::Error { .. } => {
        self.auth_status = AuthStatus::NotStarted;
      }
    };
    Ok(resp)
  }

  /// Send an auth message response to a greetd session.
  pub async fn send_auth_response(&mut self, input: Option<String>) -> GreetdResult {
    info!("Sending password to greetd");
    let msg = Request::PostAuthMessageResponse { response: input };
    msg.write_to(&mut self.socket).await?;

    let resp = Response::read_from(&mut self.socket).await?;
    match resp {
      Response::Success => {
        self.auth_status = AuthStatus::Done;
      }
      Response::AuthMessage { .. } => {
        self.auth_status = AuthStatus::InProgress;
      }
      Response::Error { .. } => {
        self.auth_status = AuthStatus::InProgress;
      }
    };
    Ok(resp)
  }

  /// Schedule starting a greetd session.
  ///
  /// On success, the session will start when this greeter terminates.
  pub async fn start_session(
    &mut self,
    command: Vec<String>,
    environment: Vec<String>,
  ) -> GreetdResult {
    info!("Starting greetd session with command: {command:?}");
    let msg = Request::StartSession {
      cmd: command,
      env: environment,
    };
    msg.write_to(&mut self.socket).await?;

    let resp = Response::read_from(&mut self.socket).await?;
    if let Response::AuthMessage { .. } = resp {
      unimplemented!("greetd responded with auth request after requesting session start.");
    }
    Ok(resp)
  }

  /// Cancel an initialized greetd session.
  pub async fn cancel_session(&mut self) -> GreetdResult {
    info!("Cancelling greetd session");
    self.auth_status = AuthStatus::NotStarted;

    let msg = Request::CancelSession;
    msg.write_to(&mut self.socket).await?;

    let resp = Response::read_from(&mut self.socket).await?;
    if let Response::AuthMessage { .. } = resp {
      unimplemented!("greetd responded with auth request after requesting session cancellation.");
    }
    Ok(resp)
  }

  pub fn get_auth_status(&self) -> &AuthStatus {
    &self.auth_status
  }
}
