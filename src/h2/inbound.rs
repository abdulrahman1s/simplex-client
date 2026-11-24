//! Network bytes into the connection: whole frames handled in order, the response finished at END_STREAM.
use super::client::H2Client;
use super::error::H2Error;
use super::frames::DEFAULT_MAX_FRAME;

impl H2Client {
    /// Takes bytes from the network, any amount. After an error the connection is dead.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), H2Error> {
        if self.dead {
            return Err(H2Error::Closed);
        }
        self.reader.push(bytes);
        let fed = self.process();
        if fed.is_err() {
            self.abandon();
        }
        fed
    }

    fn process(&mut self) -> Result<(), H2Error> {
        while let Some((header, payload)) = self.reader.take(DEFAULT_MAX_FRAME)? {
            let mut ex = self.ex.take();
            let handled = self.on_frame(ex.as_mut(), header, &payload);
            self.ex = ex;
            handled?;
            if let Some(ex) = &self.ex {
                self.progress = ex.progress();
            }
            if self.ex.as_ref().is_some_and(|e| e.ended) {
                self.finish()?;
            }
        }
        Ok(())
    }

    /// The response is complete: what was not sent of the body is cancelled.
    fn finish(&mut self) -> Result<(), H2Error> {
        let Some(ex) = self.ex.take() else {
            return Ok(());
        };
        self.done = Some(ex.into_response(&mut self.out)?);
        Ok(())
    }
}
