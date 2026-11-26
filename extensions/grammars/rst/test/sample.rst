==================
 Widget User Guide
==================

:Author: Ada Lovelace
:Version: 1.2
:License: MIT

.. This comment is not rendered.

.. |project| replace:: Widget

Introduction
============

|project| makes *small* tools with **strong** defaults. Install it with
``pip install widget``, then read the `project page <https://example.com/widget>`_
or the docs at https://example.com/docs. See :ref:`usage` for details, and
:pep:`8` for the style it follows [1]_.

.. _usage:

Usage
-----

Things to know:

    A widget is a small tool.

    -- The widget manual

- Widgets are cheap to make.
- Widgets are cheap to throw away.

#. Make a widget.
#. Use it.
#. Throw it away.

term
   A word with a definition.

.. note::

   Widgets are **not** thread safe: give each thread
   its own, or guard one with a *lock*.

.. tip:: Keep widgets small.

.. warning::
   :class: urgent

   A widget thrown away is **gone** for good.

.. code-block:: python

   from widget import Widget

   def main():
       return Widget(name="demo").run()

The area of a circle is :math:`\pi r^2`, and in general:

.. math::

   \int_0^1 x^2 \, dx = \frac{1}{3}

A literal block follows::

   $ widget --help
   usage: widget [options]

An interactive session:

>>> from widget import Widget
>>> Widget(name="demo").name
'demo'

----------

.. [1] Python's style guide, https://peps.python.org/pep-0008/.
