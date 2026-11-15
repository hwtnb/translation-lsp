# Retry only if the remote service
# reports a temporary failure.
defmodule Retry do
  @moduledoc """
  Sends requests again after a temporary failure.
  The wait doubles after each attempt.
  """

  @retry_count 3

  @doc """
  Returns the delay before the next retry, in milliseconds.

  ## Examples

      iex> Retry.delay(2)
      800
  """
  def delay(attempt), do: 200 * Integer.pow(2, attempt)

  @doc "Tells whether a status asks for another attempt."
  def temporary?(status), do: status == 503 # 503 means "try again later".

  @typedoc ~S"""
  A status code, such as 503. Find one in a log with ~r/\d{3}/.
  """
  @type status :: pos_integer()

  @doc false
  def attempts, do: @retry_count

  @doc since: "1.0.0"
  def separator, do: "# not a comment"

  def message(name) do
    "Retry #{name}: # not a comment either #{
      delay(1) # The delay of the first retry.
    } ms"
  end

  def issue?(text), do: text =~ ~r/#\d+/ or String.starts_with?(text, <<?#>>)
end
