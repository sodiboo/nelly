use halcyon_embedder::multiplexed_platform_request;

mod shutdown;

multiplexed_platform_request!(
    pub(crate) enum NellyPlatformRequest {
        type State = crate::Nelly;

        @single {
            Shutdown(shutdown::Shutdown),
        }
    }
);
