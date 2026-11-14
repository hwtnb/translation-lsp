;; Retry only if the remote service
;; reports a temporary failure.
(def retry_count 3)

(defn retry-delay
  "The docstring is not translated on Hover."
  [attempt]
  ;; The delay doubles with each attempt.
  (* 200 (bit-shift-left 1 attempt))) ; The first delay is 200 ms.

(def separator "; not a comment")
(def semicolon \;) ; A character literal is no comment either.
