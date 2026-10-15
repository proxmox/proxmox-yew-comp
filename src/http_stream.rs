use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::Poll;

use anyhow::{format_err, Context as _, Error};
use futures::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead};
use serde::Deserialize;
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

use pwt::convert_js_error;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Record<T> {
    /// A successful record.
    Data(T),
    /// An error entry.
    Error(Value),
}

impl<T> Record<T> {
    pub fn into_result(self) -> Result<T, Error> {
        match self {
            Self::Data(data) => Ok(data),
            Self::Error(Value::String(s)) => Err(Error::msg(s)),
            Self::Error(other) => match serde_json::to_string(&other) {
                Ok(s) => Err(Error::msg(s)),
                Err(err) => Err(Error::from(err)),
            },
        }
    }
}

pub struct Stream {
    reader: ReadableStreamReader,
    linebuf: Vec<u8>,
}

impl From<ReadableStreamReader> for Stream {
    fn from(reader: ReadableStreamReader) -> Self {
        Self {
            reader,
            linebuf: Vec::new(),
        }
    }
}

impl TryFrom<web_sys::ReadableStream> for Stream {
    type Error = Error;

    fn try_from(reader: web_sys::ReadableStream) -> Result<Self, Error> {
        Ok(Self::from(ReadableStreamReader::try_from(reader)?))
    }
}

impl Stream {
    pub async fn next<T>(&mut self) -> Result<Option<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        loop {
            if !read_record(&mut self.reader, &mut self.linebuf).await? {
                return Ok(None);
            }

            let entry = std::str::from_utf8(&self.linebuf)
                .map_err(|_| format_err!("non-utf8 data in stream item"))?;

            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }

            let entry: Record<T> =
                serde_json::from_str(entry).context("bad json data in snapshot list item")?;
            return entry.into_result().map(Some);
        }
    }
}

/// Read the next record of an RFC 7464 JSON text sequence into `buf`, without its framing.
/// Returns `false` at the end of the stream.
///
/// A record is a record separator (`\x1E`), the JSON text and a line feed. Waiting for the next
/// record separator holds the latest record back for as long as the sender has nothing new to
/// write, so a record is also complete at a line feed that ends a whole JSON text. Line feeds
/// inside a JSON text, as in pretty-printed output, do not end the record. A record separator
/// always ends it, so a truncated JSON text does not swallow the record that follows.
async fn read_record<R>(reader: &mut R, buf: &mut Vec<u8>) -> io::Result<bool>
where
    R: AsyncBufRead + Unpin,
{
    buf.clear();
    let mut read_any = false;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        read_any = true;
        let (used, done) = match available.iter().position(|&b| b == b'\n' || b == b'\x1E') {
            None => {
                buf.extend_from_slice(available);
                (available.len(), false)
            }
            Some(pos) if available[pos] == b'\n' => {
                buf.extend_from_slice(&available[..=pos]);
                (pos + 1, is_complete_json(buf))
            }
            Some(pos) => {
                buf.extend_from_slice(&available[..pos]);
                if buf.iter().all(u8::is_ascii_whitespace) {
                    // the separator starts this record
                    buf.clear();
                    (pos + 1, false)
                } else {
                    // the separator starts the next record, leave it for the next call
                    (pos, true)
                }
            }
        };
        reader.consume_unpin(used);
        if done {
            break;
        }
    }
    while buf.last().is_some_and(u8::is_ascii_whitespace) {
        buf.pop();
    }
    Ok(read_any)
}

/// Whether `text` holds a whole JSON text rather than the start of one.
fn is_complete_json(text: &[u8]) -> bool {
    match serde_json::from_slice::<serde::de::IgnoredAny>(text) {
        Ok(_) => true,
        // a syntax error is reported by the caller, more data would not fix it
        Err(err) => !err.is_eof(),
    }
}

pub struct ReadableStreamReader {
    reader: Option<web_sys::ReadableStreamDefaultReader>,
    read_future: Option<ReadFuture>,
    buf_at: Option<(Vec<u8>, usize)>,
}

impl ReadableStreamReader {
    pub fn records(self) -> Records {
        self.into()
    }

    pub fn json_records<T>(self) -> JsonRecords<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.into()
    }
}

impl TryFrom<web_sys::ReadableStream> for ReadableStreamReader {
    type Error = Error;

    fn try_from(reader: web_sys::ReadableStream) -> Result<Self, Error> {
        Ok(Self {
            reader: Some(
                web_sys::ReadableStreamDefaultReader::new(&reader).map_err(convert_js_error)?,
            ),
            read_future: None,
            buf_at: None,
        })
    }
}

type ReadFuture =
    Pin<Box<dyn Future<Output = (web_sys::ReadableStreamDefaultReader, Result<JsValue, Error>)>>>;

impl AsyncRead for ReadableStreamReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        use std::io::Read;
        let mut current_data = std::task::ready!(self.as_mut().poll_fill_buf(cx))?;
        let nread = current_data.read(buf)?;
        self.consume(nread);
        Poll::Ready(Ok(nread))
    }
}

impl AsyncBufRead for ReadableStreamReader {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<io::Result<&[u8]>> {
        let Self {
            ref mut reader,
            ref mut read_future,
            ref mut buf_at,
        } = Pin::into_inner(self);
        loop {
            // If we currently have a buffer, use it:
            if let Some((buf, at)) = buf_at {
                return Poll::Ready(Ok(&buf[*at..]));
            };

            // If we currently have no read future, create one.
            if read_future.is_none() {
                let Some(reader) = reader.take() else {
                    // if we have no reader, we already reached EOF
                    return Poll::Ready(Ok(&[]));
                };

                *read_future = Some(Box::pin(async move {
                    match wasm_bindgen_futures::JsFuture::from(reader.read()).await {
                        Ok(data) => (reader, Ok(data)),
                        Err(err) => (reader, Err(convert_js_error(err))),
                    }
                }));
            }

            // Now poll the read future.
            let result = match read_future.as_mut() {
                Some(future) => {
                    let (reader_inner, result) = std::task::ready!(future.as_mut().poll(cx));
                    *read_future = None;
                    *reader = Some(reader_inner);
                    result.map_err(io::Error::other)?
                }
                // if we still have no read-future, we're already at/past EOF (this should actually
                // be unreachable, since if we do have a reader, we create a read future, if we
                // don't, we return early above already)
                None => return Poll::Ready(Ok(&[])),
            };

            // We get a `{ done: bool, value: Uint8Array }` from the read future.
            if js_sys::Reflect::get(&result, &"done".into())
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(true)
            {
                // If we're done, drop the reader, so that we're at EOF once we have no more
                // remaining data.
                *reader = None;
            }

            match js_sys::Reflect::get(&result, &"value".into()) {
                Err(_) => {
                    // There's no value, we probably just got `{ done: true }` and nothing else.
                    // Just in case, retry from above in case it was a rogue `{ done: false }`
                    // which should not be possible, I think?
                    continue;
                }
                Ok(value) => match value.dyn_into::<js_sys::Uint8Array>() {
                    Err(_) => {
                        // The 'value' was not an Uint8Array. This should not be possible. Let's
                        // just assume we reached EOF.
                        *reader = None;
                        continue;
                    }
                    Ok(buf) => {
                        *buf_at = Some((buf.to_vec(), 0));
                        continue;
                    }
                },
            }
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        if let Some((buf, at)) = self.buf_at.as_mut() {
            *at = (*at + amt).min(buf.len());
            if *at == buf.len() {
                self.buf_at = None;
            }
        }
    }
}

pub enum Records {
    New(ReadableStreamReader),
    #[allow(clippy::type_complexity)]
    Reading(Pin<Box<dyn Future<Output = io::Result<Option<(Vec<u8>, ReadableStreamReader)>>>>>),
    Done,
}

impl From<ReadableStreamReader> for Records {
    fn from(reader: ReadableStreamReader) -> Self {
        Self::New(reader)
    }
}

impl TryFrom<web_sys::ReadableStream> for Records {
    type Error = Error;

    fn try_from(reader: web_sys::ReadableStream) -> Result<Self, Error> {
        Ok(Self::from(ReadableStreamReader::try_from(reader)?))
    }
}

impl futures::Stream for Records {
    type Item = io::Result<Vec<u8>>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<io::Result<Vec<u8>>>> {
        loop {
            return match std::mem::replace(&mut *self, Self::Done) {
                Self::New(mut reader) => {
                    let fut = Box::pin(async move {
                        let mut linebuf = Vec::new();
                        loop {
                            if !read_record(&mut reader, &mut linebuf).await? {
                                return Ok(None);
                            }
                            if linebuf.is_empty() {
                                continue;
                            }
                            return Ok(Some((linebuf, reader)));
                        }
                    });
                    *self = Self::Reading(fut);
                    continue;
                }
                Self::Reading(mut fut) => match fut.as_mut().poll(cx) {
                    Poll::Pending => {
                        *self = Self::Reading(fut);
                        Poll::Pending
                    }
                    Poll::Ready(Ok(None)) => Poll::Ready(None),
                    Poll::Ready(Ok(Some((data, reader)))) => {
                        *self = Self::New(reader);
                        Poll::Ready(Some(Ok(data)))
                    }
                    Poll::Ready(Err(err)) => {
                        *self = Self::Done;
                        Poll::Ready(Some(Err(err)))
                    }
                },
                Self::Done => Poll::Ready(None),
            };
        }
    }
}

impl Records {
    pub fn json<T>(self) -> JsonRecords<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.into()
    }
}

pub struct JsonRecords<T> {
    records: Records,
    _phantom: PhantomData<fn() -> T>,
}

impl<T> futures::Stream for JsonRecords<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Item = Result<T, Error>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<T, Error>>> {
        let this = self.get_mut();
        loop {
            match std::task::ready!(Pin::new(&mut this.records).poll_next(cx)) {
                None => return Poll::Ready(None),
                Some(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
                Some(Ok(data)) => {
                    let data = std::str::from_utf8(&data)
                        .map_err(|_| format_err!("non-utf8 json data in record element"))?
                        .trim();
                    if data.is_empty() {
                        continue;
                    }
                    return Poll::Ready(Some(
                        serde_json::from_str(data).context("bad json in record element"),
                    ));
                }
            }
        }
    }
}

impl<T> From<Records> for JsonRecords<T>
where
    T: for<'de> Deserialize<'de>,
{
    fn from(records: Records) -> Self {
        Self {
            records,
            _phantom: PhantomData,
        }
    }
}

impl<T> From<ReadableStreamReader> for JsonRecords<T>
where
    T: for<'de> Deserialize<'de>,
{
    fn from(reader: ReadableStreamReader) -> Self {
        Self::from(Records::from(reader))
    }
}

impl<T> TryFrom<web_sys::ReadableStream> for JsonRecords<T>
where
    T: for<'de> Deserialize<'de>,
{
    type Error = Error;

    fn try_from(reader: web_sys::ReadableStream) -> Result<Self, Error> {
        Ok(Self::from(Records::try_from(reader)?))
    }
}

#[cfg(test)]
mod tests {
    use futures::{FutureExt, StreamExt, TryStreamExt};

    use super::read_record;

    /// The records `chunks` hold, read from a stream that stays open after them, as a response
    /// does while the sender has nothing new to write.
    fn records(chunks: &[&'static [u8]]) -> Vec<Vec<u8>> {
        let chunks: Vec<std::io::Result<&[u8]>> = chunks.iter().map(|c| Ok(*c)).collect();
        let mut reader = futures::stream::iter(chunks)
            .chain(futures::stream::pending())
            .into_async_read();
        let mut buf = Vec::new();
        let mut records = Vec::new();
        while let Some(more) = read_record(&mut reader, &mut buf).now_or_never() {
            assert!(more.unwrap());
            records.push(buf.clone());
        }
        records
    }

    #[test]
    fn json_seq_records() {
        // each record is returned as soon as it is complete, not when the next one starts
        assert_eq!(
            records(&[b"\x1E{\"data\":1}\n"]),
            [b"{\"data\":1}".to_vec()]
        );
        assert_eq!(
            records(&[b"\x1E{\"data\":1}\n\x1E{\"da", b"ta\":2}\n"]),
            [b"{\"data\":1}".to_vec(), b"{\"data\":2}".to_vec()]
        );
        // a record is not returned before its line feed arrives
        assert!(records(&[b"\x1E{\"data\":1}"]).is_empty());
        // a line feed inside a JSON text does not end the record
        assert_eq!(
            records(&[b"\x1E{\n  \"data\": 1\n", b"}\n"]),
            [b"{\n  \"data\": 1\n}".to_vec()]
        );
        // a record separator ends a truncated JSON text
        assert_eq!(
            records(&[b"\x1E{\"da\x1E{\"data\":2}\n"]),
            [b"{\"da".to_vec(), b"{\"data\":2}".to_vec()]
        );
    }
}
