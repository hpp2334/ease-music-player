macro_rules! generate_dispatch_message {
    ($($m: ty, $h: expr),*) => {
            pub(crate) async fn dispatch_message(cx: &std::sync::Arc<crate::BackendContext>, arg: ease_client_shared::backends::MessagePayload) -> crate::BResult<ease_client_shared::backends::MessagePayload> {
            #[tracing::instrument]
            fn trace_request<M: ease_client_shared::backends::IMessage>(code: ease_client_shared::backends::generated::Code, arg: &<M as ease_client_shared::backends::IMessage>::Argument) {
                tracing::trace!("request {:?}: {:?}", code, arg)
            }

            #[tracing::instrument]
            fn trace_response<M: ease_client_shared::backends::IMessage>(code: ease_client_shared::backends::generated::Code, arg: &<M as ease_client_shared::backends::IMessage>::Return) {
                tracing::trace!("response {:?}: {:?}", code, arg)
            }

            $(
                if <$m as ease_client_shared::backends::IMessage>::CODE == arg.code {
                    let code = arg.code;
                    let arg = ease_client_shared::backends::decode_message_payload::<<$m as ease_client_shared::backends::IMessage>::Argument>(arg.payload);
                    trace_request::<$m>(code, &arg);
                    let ret = $h(cx, arg).await?;
                    trace_response::<$m>(code, &ret);
                    let ret = ease_client_shared::backends::encode_message_payload(ret);
                    let ret = ease_client_shared::backends::MessagePayload {
                        code,
                        payload: ret,
                    };
                    return Ok(ret);
                }
            )*
            return Err(crate::error::BError::NoSuchMessage(arg.code));
        }
    };
}
