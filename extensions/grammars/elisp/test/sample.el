;;; sample.el --- A sample of Emacs Lisp for the highlight golden  -*- lexical-binding: t; -*-

;; A file-level comment, written with two semicolons as convention has it.
;; This sample exercises the token classes the grammar distinguishes; it is
;; not meant to be a useful package.

(require 'subr-x)
(require 'cl-lib)

;;; Variables

(defvar sample-history nil
  "Entries the user has visited, newest first.")

(defconst sample-separator ?\t
  "Character written between a label and its value.")

(defcustom sample-limit 64
  "How many entries `sample-remember' keeps."
  :type 'integer
  :group 'sample)

(defcustom sample-greeting "Hello,\tworld\n"
  "Text shown on an empty history.  It contains an escape or two."
  :type 'string
  :group 'sample)

;;; Macros

(defmacro sample-with-buffer (name &rest body)
  "Evaluate BODY in a buffer called NAME, creating it if need be."
  `(let ((buffer (get-buffer-create ,name)))
     (with-current-buffer buffer
       ,@body
       buffer)))

(defmacro sample-ignore-errors (&rest body)
  "Run BODY, returning nil if it signals."
  `(condition-case nil
       (progn ,@body)
     (error nil)))

;;; Functions

(defun sample-remember (entry &optional limit)
  "Add ENTRY to `sample-history', keeping at most LIMIT entries.
LIMIT defaults to `sample-limit'.  Returns the new history."
  (let* ((limit (or limit sample-limit))
         (seen (delete entry sample-history)))
    (setq sample-history (cons entry seen))
    (when (> (length sample-history) limit)
      (setq sample-history (butlast sample-history)))
    sample-history))

(defun sample-describe (entry)
  "Return a one-line description of ENTRY."
  (cond
   ((null entry) sample-greeting)
   ((stringp entry) (concat "string: " entry))
   ((numberp entry) (format "number: %d" entry))
   (t (format "other: %S" entry))))

(defsubst sample-first-word (text)
  "Return the first whitespace-delimited word of TEXT."
  (car (split-string text nil t)))

(defun sample-report (&optional entries)
  "Describe ENTRIES, defaulting to `sample-history'."
  (unless entries
    (setq entries sample-history))
  (mapconcat #'sample-describe entries (string sample-separator)))

(defun sample-sorted-labels (alist)
  "Return the labels of ALIST, sorted."
  (sort (mapcar #'car alist)
        (lambda (a b) (string< (symbol-name a) (symbol-name b)))))

(defun sample-demo ()
  "Exercise this file's own definitions."
  (interactive)
  (sample-with-buffer "*sample*"
    (sample-remember '(a b))
    (sample-remember "text")
    (sample-remember 1.5)
    (insert (sample-report))
    (insert (sample-first-word "  two words"))
    (sample-ignore-errors (error "never seen"))
    (sample-sorted-labels '((zeta . 1) (alpha . 2)))))

(provide 'sample)
;;; sample.el ends here
