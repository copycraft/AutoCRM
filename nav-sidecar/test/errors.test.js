/**
 * The error translation, which is the contract the Rust backend logs against.
 *
 * The `nav_rejected` path cannot be reached through the mock — the mock decides
 * an invoice's fate with the same validator the sidecar runs before submitting,
 * so anything it would abort is refused locally first. It is reached against the
 * real service, where NAV knows things no local check can (an invoice number
 * already used, a taxpayer struck off), so it is covered here directly.
 */
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { NavApiError, NavInvoiceRejectedError, NavTransportError } from '@open-nav/core';
import { toApiError } from '../dist/errors.js';

describe('toApiError', () => {
  it("keeps NAV's fault code and messages when an invoice is rejected", () => {
    const error = new NavInvoiceRejectedError('4Q7ZXY8K2M1N', [
      {
        index: 1,
        invoiceStatus: 'ABORTED',
        messages: [
          {
            validationResultCode: 'ERROR',
            validationErrorCode: 'INVOICE_NUMBER_ALREADY_EXISTS',
            message: 'Invoice number already exists',
            tag: '/InvoiceData/invoiceNumber',
          },
        ],
      },
    ]);

    const api = toApiError(error);
    assert.equal(api.status, 422);

    const { error: body } = api.body();
    assert.equal(body.kind, 'nav_rejected');
    assert.equal(body.transactionId, '4Q7ZXY8K2M1N');
    assert.deepEqual(body.messages, [
      {
        source: 'business',
        level: 'ERROR',
        code: 'INVOICE_NUMBER_ALREADY_EXISTS',
        message: 'Invoice number already exists',
        path: '/InvoiceData/invoiceNumber',
      },
    ]);
  });

  it('surfaces a NAV fault as a 502 carrying its own codes', () => {
    const api = toApiError(
      new NavApiError({
        message: 'NAV rejected the request: INVALID_SECURITY_USER',
        status: 401,
        funcCode: 'ERROR',
        errorCode: 'INVALID_SECURITY_USER',
        validationMessages: [
          { validationResultCode: 'ERROR', validationErrorCode: 'INVALID_SECURITY_USER' },
        ],
        responseBody: '<GeneralErrorResponse/>',
      }),
    );

    assert.equal(api.status, 502);
    const { error: body } = api.body();
    assert.equal(body.kind, 'nav_error');
    assert.equal(body.navErrorCode, 'INVALID_SECURITY_USER');
    assert.equal(body.navFuncCode, 'ERROR');
    // The raw XML stays out of the response; the code and message are what matter.
    assert.equal(JSON.stringify(body).includes('GeneralErrorResponse'), false);
  });

  it('calls an unreachable NAV a 504, not a 500', () => {
    const api = toApiError(new NavTransportError('connect ETIMEDOUT'));
    assert.equal(api.status, 504);
    assert.equal(api.body().error.kind, 'nav_unreachable');
  });

  it('always answers with a messages array', () => {
    const api = toApiError(new Error('something unexpected'));
    assert.equal(api.status, 500);
    assert.deepEqual(api.body().error.messages, []);
    assert.equal(api.body().error.kind, 'internal');
  });
});
