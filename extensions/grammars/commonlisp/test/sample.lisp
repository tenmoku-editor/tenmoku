#!/usr/bin/env sbcl --script
;;;; inventory.lisp -- a small stock keeper.

#|
Load with Quicklisp, then call (inventory:main).
A block comment may span several lines.
|#

(defpackage :inventory
  (:use :cl)
  (:export #:main #:add-item #:total-value))

(in-package :inventory)

(defvar *stock* (make-hash-table :test #'equal)
  "Every item, keyed by name.")

(defparameter *log-stream* *standard-output*)

(defconstant +tax-rate+ 1/8)
(defconstant +scale+ 1.5d0)

(defclass item ()
  ((name :initarg :name :reader item-name :type string)
   (price :initarg :price :accessor item-price :initform 0)
   (count :initarg :count :accessor item-count :initform 1))
  (:documentation "One kind of thing in stock."))

(defgeneric value (thing &key taxed)
  (:documentation "What THING is worth."))

(defmethod value ((thing item) &key (taxed nil))
  (let* ((base (* (item-price thing) (item-count thing)))
         (tax (if taxed (* base +tax-rate+) 0)))
    (+ base tax)))

(defmethod report ((thing item) &optional (stream *log-stream*) (loud t)
                   &key (rate +tax-rate+) (label fallback))
  (when loud
    (format stream "~a: ~a at ~a~%" label (item-name thing) rate)))

(defun add-item (name price &optional (count 1) &rest tags)
  "Add an item called NAME to *STOCK*."
  (declare (ignore tags))
  (setf (gethash name *stock*)
        (make-instance 'item :name name :price price :count count)))

(defmacro with-logging ((label &key (level :info)) &body body)
  `(progn
     (format *log-stream* "~&[~a] ~a~%" ,level ,label)
     ,@body))

(defun total-value ()
  (loop for item being the hash-values of *stock*
        sum (value item :taxed t)))

(defun item-names ()
  (loop for name being the hash-keys of *stock*
        collect name into names
        finally (return (sort names #'string<))))

(defun initials (names)
  (loop for name in names
        when (plusp (length name))
          collect (char name 0)))

(defun separator-p (ch)
  (member ch '(#\a #\Space #\, #\;)))

(defun main ()
  (with-logging ("startup")
    (add-item "apple" 0.5 10)
    (add-item "pear \"ripe\"" 3/4))
  (let ((sizes #(1 2 3))
        (mask #xFF)
        (words '(alpha beta gamma)))
    (format t "Total: ~,2f over ~d item~:p~%" (total-value)
            (hash-table-count *stock*))
    (mapcar (lambda (word) (list word (length (symbol-name word))))
            words)
    (values (aref sizes 0) mask (- (get-universal-time) 100))))
