import {
  NavApiError,
  NavInvoiceRejectedError,
  NavTransportError,
  NavValidationError,
} from '@open-nav/core';
import type { InvoiceValidationIssue, ProcessingResultType } from '@open-nav/core';
import type { NextFunction, Request, Response } from 'express';
import { ZodError } from 'zod';
import { ConfigError } from './config.js';

/**
 * Why a request failed, as a small closed set.
 *
 * The Rust client branches on this, so the set is part of the contract: new
 * kinds may be added, existing ones do not change meaning.
 */
export type ErrorKind =
  /** The request body or path did not match the documented shape. */
  | 'bad_request'
  /** The document is well-formed but NAV would reject it; caught locally. */
  | 'validation'
  /** NAV accepted the batch and then rejected the invoice (ABORTED). */
  | 'nav_rejected'
  /** NAV answered with a fault: bad credentials, bad signature, unknown operation. */
  | 'nav_error'
  /** NAV could not be reached, or did not settle the transaction in time. */
  | 'nav_unreachable'
  /** No such invoice, or no such rendered proforma. */
  | 'not_found'
  /** The sidecar is misconfigured. */
  | 'config'
  /** Anything else. */
  | 'internal';

/**
 * One finding, wherever it came from.
 *
 * NAV splits its verdict across technical and business validation messages,
 * and the local validator adds its own; they are flattened into one list so
 * the caller has a single thing to log and show. `code` is NAV's own fault
 * code whenever one exists.
 */
export interface ApiMessage {
  /** `technical` and `business` are NAV's; `local` is this service's validator. */
  source: 'technical' | 'business' | 'local' | 'request';
  level: 'ERROR' | 'WARN' | 'INFO';
  /** NAV fault code, where there is one. */
  code?: string;
  message: string;
  /** Field this concerns, dotted: `lines.0.vatPercentage`. */
  path?: string;
}

export interface ApiErrorBody {
  error: {
    kind: ErrorKind;
    message: string;
    /** NAV's `result/errorCode`, verbatim. */
    navErrorCode?: string;
    /** NAV's `result/funcCode`, verbatim. */
    navFuncCode?: string;
    /** Set whenever the failure happened after a batch was accepted. */
    transactionId?: string;
    /** Always present, possibly empty. */
    messages: ApiMessage[];
  };
}

/** An error that already knows how it should surface over HTTP. */
export class ApiError extends Error {
  readonly status: number;
  readonly kind: ErrorKind;
  readonly messages: ApiMessage[];
  readonly navErrorCode: string | undefined;
  readonly navFuncCode: string | undefined;
  readonly transactionId: string | undefined;

  constructor(init: {
    status: number;
    kind: ErrorKind;
    message: string;
    messages?: ApiMessage[];
    navErrorCode?: string;
    navFuncCode?: string;
    transactionId?: string;
    cause?: unknown;
  }) {
    super(init.message, init.cause === undefined ? undefined : { cause: init.cause });
    this.name = 'ApiError';
    this.status = init.status;
    this.kind = init.kind;
    this.messages = init.messages ?? [];
    this.navErrorCode = init.navErrorCode;
    this.navFuncCode = init.navFuncCode;
    this.transactionId = init.transactionId;
  }

  body(): ApiErrorBody {
    return {
      error: {
        kind: this.kind,
        message: this.message,
        ...(this.navErrorCode ? { navErrorCode: this.navErrorCode } : {}),
        ...(this.navFuncCode ? { navFuncCode: this.navFuncCode } : {}),
        ...(this.transactionId ? { transactionId: this.transactionId } : {}),
        messages: this.messages,
      },
    };
  }
}

export function badRequest(message: string, messages: ApiMessage[] = []): ApiError {
  return new ApiError({ status: 400, kind: 'bad_request', message, messages });
}

export function notFound(message: string): ApiError {
  return new ApiError({ status: 404, kind: 'not_found', message });
}

/** Local validation findings, carrying NAV's fault codes where they exist. */
export function validationFailed(issues: InvoiceValidationIssue[], what: string): ApiError {
  return new ApiError({
    status: 422,
    kind: 'validation',
    message: `${what} would be rejected by NAV: ${issues[0]?.message ?? 'invalid'}`,
    messages: issues.map((issue) => ({
      source: 'local' as const,
      level: issue.severity === 'error' ? ('ERROR' as const) : ('WARN' as const),
      code: issue.code,
      message: issue.navMessage ? `${issue.message} (${issue.navMessage})` : issue.message,
      path: issue.path,
    })),
  });
}

/** Flatten NAV's per-invoice verdict into the flat message list. */
export function resultMessages(result: ProcessingResultType): ApiMessage[] {
  return [
    ...(result.technicalValidationMessages ?? []).map((message) => ({
      source: 'technical' as const,
      level: normaliseLevel(message.validationResultCode),
      ...(message.validationErrorCode ? { code: message.validationErrorCode } : {}),
      message: message.message ?? message.validationResultCode,
    })),
    ...(result.businessValidationMessages ?? []).map((message) => ({
      source: 'business' as const,
      level: normaliseLevel(message.validationResultCode),
      ...(message.validationErrorCode ? { code: message.validationErrorCode } : {}),
      message: message.message ?? message.validationResultCode,
      ...(message.pointer?.tag ? { path: message.pointer.tag } : {}),
    })),
  ];
}

function normaliseLevel(code: string): ApiMessage['level'] {
  return code === 'ERROR' || code === 'WARN' || code === 'INFO' ? code : 'ERROR';
}

/**
 * Turn anything thrown into the wire shape, keeping NAV's own words.
 *
 * The point of the sidecar is that the Rust backend can log and show what NAV
 * actually said, so a fault code is never folded into a generic 500 here.
 */
export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;

  if (error instanceof ZodError) {
    return new ApiError({
      status: 400,
      kind: 'bad_request',
      message: 'request body does not match the documented shape',
      // An unknown key names itself in `keys`, not in `path`; fold it into the
      // path so the caller is told which field it should not have sent.
      messages: error.issues.flatMap((issue): ApiMessage[] =>
        issue.code === 'unrecognized_keys'
          ? issue.keys.map((key) => ({
              source: 'request' as const,
              level: 'ERROR' as const,
              code: issue.code,
              message: `unrecognised field ${key}; this service takes only the documented fields`,
              path: [...issue.path, key].join('.'),
            }))
          : [
              {
                source: 'request' as const,
                level: 'ERROR' as const,
                code: issue.code,
                message: issue.message,
                path: issue.path.join('.'),
              },
            ],
      ),
      cause: error,
    });
  }

  if (error instanceof NavInvoiceRejectedError) {
    return new ApiError({
      status: 422,
      kind: 'nav_rejected',
      message: error.message,
      transactionId: error.transactionId,
      messages: error.rejected.flatMap((entry) =>
        entry.messages.map((message) => ({
          source: 'business' as const,
          level: normaliseLevel(message.validationResultCode),
          ...(message.validationErrorCode ? { code: message.validationErrorCode } : {}),
          message: message.message ?? message.validationResultCode,
          ...(message.tag ? { path: message.tag } : {}),
        })),
      ),
      cause: error,
    });
  }

  if (error instanceof NavApiError) {
    return new ApiError({
      status: 502,
      kind: 'nav_error',
      message: error.message,
      ...(error.errorCode ? { navErrorCode: error.errorCode } : {}),
      ...(error.funcCode ? { navFuncCode: error.funcCode } : {}),
      messages: error.validationMessages.map((message) => ({
        source: message.tag === undefined ? ('technical' as const) : ('business' as const),
        level: normaliseLevel(message.validationResultCode),
        ...(message.validationErrorCode ? { code: message.validationErrorCode } : {}),
        message: message.message ?? message.validationResultCode,
        ...(message.tag ? { path: message.tag } : {}),
      })),
      cause: error,
    });
  }

  if (error instanceof NavTransportError) {
    return new ApiError({
      status: 504,
      kind: 'nav_unreachable',
      message: error.message,
      cause: error,
    });
  }

  if (error instanceof NavValidationError) {
    return new ApiError({
      status: 422,
      kind: 'validation',
      message: error.message,
      messages: error.issues.map((issue) => ({
        source: 'local' as const,
        level: 'ERROR' as const,
        ...(issue.code ? { code: issue.code } : {}),
        message: issue.message,
        path: issue.path,
      })),
      cause: error,
    });
  }

  if (error instanceof ConfigError) {
    return new ApiError({ status: 500, kind: 'config', message: error.message, cause: error });
  }

  // Body parser failures (malformed JSON, body over the limit) arrive as
  // http-errors with their own status; keep it rather than calling them 500s.
  const status = bodyParserStatus(error);
  if (status !== undefined) {
    return new ApiError({
      status,
      kind: 'bad_request',
      message: error instanceof Error ? error.message : 'request body could not be read',
      cause: error,
    });
  }

  return new ApiError({
    status: 500,
    kind: 'internal',
    message: error instanceof Error ? error.message : String(error),
    cause: error,
  });
}

function bodyParserStatus(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const candidate = error as { status?: unknown; statusCode?: unknown; type?: unknown };
  if (typeof candidate.type !== 'string') return undefined;
  const status = typeof candidate.status === 'number' ? candidate.status : candidate.statusCode;
  return typeof status === 'number' && status >= 400 && status < 500 ? status : undefined;
}

/** Express error handler. Keeps all four parameters, or Express ignores it. */
export function errorHandler(
  error: unknown,
  _request: Request,
  response: Response,
  next: NextFunction,
): void {
  if (response.headersSent) {
    next(error);
    return;
  }
  const api = toApiError(error);
  if (api.kind === 'internal') {
    // Only a genuine bug gets the stack; a NAV fault is already fully described
    // by the response, and its raw XML body would bury the log.
    console.error('[nav-sidecar] internal error:', api.cause ?? api);
  } else if (api.status >= 500) {
    console.error(`[nav-sidecar] ${api.kind}: ${api.message}`);
  }
  response.status(api.status).json(api.body());
}
