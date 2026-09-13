#!/usr/bin/env python3
"""Finish E014 parity with its frozen evaluator and canonical JSON signatures.

The initial evaluator generated tuples in its reference signature, whereas
JSON reload returns lists. Normalize only that JSON representation before
calling the original strict guard. Every binary/input/command/evaluator/helper
hash is still checked. Keep the executed evaluator and adapter identities.
"""
import importlib.util
import json
from pathlib import Path


def canonical_signature(signature):
    return json.loads(json.dumps(signature, allow_nan=False))


def main():
    frozen=Path('target/continuation/frozen-evaluator/continuation_eval.py')
    spec=importlib.util.spec_from_file_location('continuation_executed',frozen)
    c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
    original=c.validate_resume
    def validate(row,signature):
        original(row,canonical_signature(signature))
    c.validate_resume=validate
    c.save(c.ROOT/'resume-adapter.json',dict(adapter=c.e.identity(__file__),
        executed_evaluator=c.e.identity(frozen),
        cause='JSON serialization turns reference tuples into lists; all identity values match',
        normalization='JSON round trip only; original strict guard then checks every field and output hash'))
    c.run((c.ROOT/'candidate-v3/candidate').resolve(),'parity')


if __name__=='__main__':main()
