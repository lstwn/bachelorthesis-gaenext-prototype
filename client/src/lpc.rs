use tokio::sync::mpsc;
use tokio::sync::oneshot;

pub fn setup<Args, Return>(
    buffer: usize,
) -> (CallerHandle<Args, Return>, CalleeHandle<Args, Return>) {
    let (tx, rx) = mpsc::channel(buffer);
    let caller_handle = CallerHandle::new(tx);
    let callee_handle = CalleeHandle::new(rx);
    (caller_handle, callee_handle)
}

pub struct CalleeHandle<Args, Return> {
    callers: mpsc::Receiver<LocalProcedureCall<Args, Return>>,
}

impl<Args, Return> CalleeHandle<Args, Return> {
    fn new(callers: mpsc::Receiver<LocalProcedureCall<Args, Return>>) -> Self {
        Self { callers }
    }
}

#[derive(Clone)]
pub struct CallerHandle<Args, Return> {
    callee: mpsc::Sender<LocalProcedureCall<Args, Return>>,
}

impl<Args, Return> CallerHandle<Args, Return> {
    fn new(callee: mpsc::Sender<LocalProcedureCall<Args, Return>>) -> Self {
        Self { callee }
    }
    pub async fn request(
        &self,
        args: Args,
    ) -> Result<Return, mpsc::error::SendError<LocalProcedureCall<Args, Return>>> {
        let (tx, rx) = oneshot::channel();
        self.callee.send(LocalProcedureCall::new(tx, args)).await?;
        Ok(rx.await?)
    }
}

// TODO error type

pub struct LocalProcedureCall<Args, Return> {
    reply_to: oneshot::Sender<Return>,
    args: Args,
}

impl<Args, Return> LocalProcedureCall<Args, Return> {
    fn new(tx: oneshot::Sender<Return>, args: Args) -> Self {
        Self { reply_to: tx, args }
    }
    pub async fn reply(self, result: Return) -> Result<(), Return> {
        self.reply_to.send(result)
    }
}
