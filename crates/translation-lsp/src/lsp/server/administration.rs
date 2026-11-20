use super::*;

const ADMIN_TIMEOUT: Duration = Duration::from_secs(180);

impl<P: TranslationProvider> Server<P> {
    pub(super) fn cancel_actions(
        &self,
        params: &CodeActionParams,
    ) -> RequestResult<Vec<CodeAction>> {
        let mut actions = Vec::new();
        let mut ranges: Vec<_> = self
            .scheduler
            .translations()
            .iter()
            .filter(|(_, work)| {
                work.owners.retained()
                    && work.target.uri == params.text_document.uri
                    && overlaps(work.target.range, params.range)
            })
            .collect();
        ranges.sort_by_key(|(ticket, _)| *ticket);
        // A range can hold a tab operation next to a range operation only when
        // they do not always share, so only then does the title tell them apart.
        let separate = self.sharing() != Sharing::Both;
        for (&ticket, work) in ranges {
            actions.push(ServerCommand::Cancel(ticket).action(format!(
                "Cancel {} of lines {}-{}",
                if separate && work.owners.tab() {
                    "tab translation"
                } else {
                    "translation"
                },
                work.target.range.start.line + 1,
                work.target.range.end.line + 1
            ))?);
        }
        if !self.scheduler.translations().is_empty() {
            actions.push(ServerCommand::CancelAll.action("Cancel all translations".into())?);
        }
        Ok(actions)
    }

    pub(super) fn account_change(command: &str) -> AccountChange {
        P::capabilities()
            .commands
            .iter()
            .find(|entry| entry.command == command)
            .map_or(AccountChange::Never, |entry| entry.changes_account)
    }

    pub(super) fn finish_administration(
        &mut self,
        connection: &Connection,
        completion: Completion<Administered>,
    ) -> ServerResult<()> {
        let Some(operation) = self.administrations.remove(&completion.ticket) else {
            return Ok(());
        };
        match operation.change {
            AccountChange::AtOnce => self.clear_previews(),
            // The user agreed, and the command changed what translations
            // are made with.
            AccountChange::IfConfirmed if self.provider.context().ok() != operation.context => {
                self.cancel_all_work(connection)?;
                self.clear_previews();
            }
            _ => (),
        }
        let result = match completion.result {
            _ if operation.deadline <= Instant::now() => Err(failure(
                ErrorCode::RequestFailed,
                "Account operation timed out",
            )),
            Ok(Administered::Message(message)) => {
                self.ui.inform(&message)?;
                Ok(Value::Null)
            }
            Ok(Administered::Done) => Ok(Value::Null),
            Err(error) => Err(provider_failure(error)),
        };
        if let Err(error) = &result {
            self.command_failure(error)?;
        }
        match operation.request {
            Some(id) => respond(connection, id, result),
            None => Ok(()),
        }
    }

    /// Starts a job on the administration worker and answers `request`, if
    /// any, when it finishes.
    pub(super) fn start_administration(
        &mut self,
        connection: &Connection,
        request: Option<RequestId>,
        task: Task<P::Context>,
    ) -> RequestResult<Option<Value>> {
        if !self.administrations.is_empty() {
            return Err(failure(
                ErrorCode::RequestFailed,
                "An account operation is in progress",
            ));
        }
        let change = match &task {
            Task::Administration { command, .. } => Self::account_change(command),
            _ => AccountChange::Never,
        };
        if change == AccountChange::AtOnce {
            self.cancel_all_work(connection)
                .map_err(|_| failure(ErrorCode::InternalError, "Could not cancel pending work"))?;
            self.clear_previews();
        }
        let context = (change == AccountChange::IfConfirmed)
            .then(|| self.provider.context().ok())
            .flatten();
        self.next_ticket += 1;
        let ticket = self.next_ticket;
        let deadline = Instant::now() + ADMIN_TIMEOUT;
        let (stop, receiver) = watch::channel(Some(deadline));
        self.admin_jobs
            .as_ref()
            .unwrap()
            .try_send(Job {
                ticket,
                deadline: receiver,
                task,
            })
            .map_err(|_| {
                failure(
                    ErrorCode::RequestFailed,
                    "Translation worker is busy; retry the account command",
                )
            })?;
        self.administrations.insert(
            ticket,
            Administration {
                request,
                change,
                stop,
                deadline,
                context,
            },
        );
        Ok(None)
    }
}
