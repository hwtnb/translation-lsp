%%% A service may report a temporary failure.
%%% Check the response before trying again.
-module(comment).
-moduledoc """
    Retry only if the remote service
    reports a temporary failure.
    """.

-export([retry_count/0, percent/0, internal/0, version/0]).

%% Retry only if the remote service
%% reports a temporary failure.
-doc """
    Returns the number of retries to allow.

        comment:retry_count().
    """.
retry_count() ->
    2. % Keep this limit small.

-doc "Returns three ways to write a percent sign.".
percent() ->
    {"% inside a string", $%, 'percent%atom'}.

-doc false.
internal() ->
    ok.

-doc #{since => <<"1.0">>}.
version() ->
    1.
