---
title: Introduction
---

# Introduction

hermes-test is a fast, deterministic test runner for React Native and Expo.

## Why teams choose it

- Fast local loops
- Easy setup and migration
- Jest-style ergonomics with a focused API
- Real Hermes runtime execution

## Why it exists

Traditional RN Jest setups often become slow and fragile:

- Node runtime mismatch vs Hermes in production
- repeated transform overhead
- brittle config (`transformIgnorePatterns`, module mapping, mocks)

## What hermes-test changes

- One esbuild pass instead of per-test transform churn
- Single-process execution path
- Hermes runtime parity for app-level behavior
- Typed testing API designed for hooks and RN workflows

## Real-world impact

- Full suites in seconds instead of minutes (compared to common Jest-based RN setups)
- Fast reruns for tight feedback loops
- Less configuration maintenance over time

Next: [Installation](./installation)
