//
// Copyright 2024 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

package org.signal.libsignal.net;

import static org.junit.Assert.*;

import java.time.Duration;
import java.util.EnumSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ExecutionException;
import org.junit.Test;
import org.signal.libsignal.internal.NativeTesting;

public class RegistrationServiceTest {

  private static class FakeRegistrationException extends Exception {}

  private interface ThrowingConsumer<T> {
    void accept(T value) throws Exception;
  }

  @Test
  public void testConvertRegistrationSessionInfo() throws Exception {
    var info = new RegistrationSessionState(NativeTesting.TESTING_RegistrationSessionInfoConvert());
    assertEquals(info.getAllowedToRequestCode(), true);
    assertEquals(info.getVerified(), true);
    assertEquals(info.getNextCall(), Duration.ofSeconds(123));
    assertEquals(info.getNextSms(), Duration.ofSeconds(456));
    assertEquals(info.getNextVerificationAttempt(), Duration.ofSeconds(789));
    assertEquals(
        info.getRequestedInformation(),
        EnumSet.of(RegistrationSessionState.RequestedInformation.PUSH_CHALLENGE));
  }

  @Test
  public void testCreateSessionErrorConversion() {
    assertRegistrationSessionErrorIs(
        "InvalidSessionId",
        RegistrationSessionIdInvalidException.class,
        NativeTesting::TESTING_RegistrationService_CreateSessionErrorConvert);
    assertIsRetryAfterError(NativeTesting::TESTING_RegistrationService_CreateSessionErrorConvert);
    assertIsTimeoutError(NativeTesting::TESTING_RegistrationService_CreateSessionErrorConvert);
    assertIsUnknownError(NativeTesting::TESTING_RegistrationService_CreateSessionErrorConvert);
  }

  @Test
  public void testResumeSessionErrorConversion() {
    assertRegistrationSessionErrorIs(
        "InvalidSessionId",
        RegistrationSessionIdInvalidException.class,
        NativeTesting::TESTING_RegistrationService_ResumeSessionErrorConvert);
    assertRegistrationSessionErrorIs(
        "SessionNotFound",
        RegistrationSessionNotFoundException.class,
        NativeTesting::TESTING_RegistrationService_ResumeSessionErrorConvert);
    assertIsTimeoutError(NativeTesting::TESTING_RegistrationService_ResumeSessionErrorConvert);
    assertIsUnknownError(NativeTesting::TESTING_RegistrationService_ResumeSessionErrorConvert);
  }

  @Test
  public void testUpdateSessionErrorConversion() {
    assertRegistrationSessionErrorIs(
        "Rejected",
        RegistrationException.class,
        NativeTesting::TESTING_RegistrationService_UpdateSessionErrorConvert);
    assertIsRetryAfterError(NativeTesting::TESTING_RegistrationService_UpdateSessionErrorConvert);
    assertIsTimeoutError(NativeTesting::TESTING_RegistrationService_UpdateSessionErrorConvert);
    assertIsUnknownError(NativeTesting::TESTING_RegistrationService_UpdateSessionErrorConvert);
  }

  @Test
  public void testRequestVerificationCodeErrorConversion() {
    assertRegistrationSessionErrorIs(
        "InvalidSessionId",
        RegistrationSessionIdInvalidException.class,
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertRegistrationSessionErrorIs(
        "SessionNotFound",
        RegistrationSessionNotFoundException.class,
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertRegistrationSessionErrorIs(
        "NotReadyForVerification",
        RegistrationSessionNotReadyException.class,
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertRegistrationSessionErrorIs(
        "SendFailed",
        RegistrationSessionSendCodeException.class,
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);

    var notDeliverableException =
        assertRegistrationSessionErrorIs(
            "CodeNotDeliverable",
            RegistrationCodeNotDeliverableException.class,
            NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertEquals(notDeliverableException.reason, "no reason");
    assertEquals(notDeliverableException.permanentFailure, true);

    assertIsRetryAfterError(
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertIsTimeoutError(
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
    assertIsUnknownError(
        NativeTesting::TESTING_RegistrationService_RequestVerificationCodeErrorConvert);
  }

  @Test
  public void testSubmitVerificationErrorConversion() {
    assertRegistrationSessionErrorIs(
        "InvalidSessionId",
        RegistrationSessionIdInvalidException.class,
        NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
    assertRegistrationSessionErrorIs(
        "SessionNotFound",
        RegistrationSessionNotFoundException.class,
        NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
    assertRegistrationSessionErrorIs(
        "NotReadyForVerification",
        RegistrationSessionNotReadyException.class,
        NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
    assertIsRetryAfterError(
        NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
    assertIsTimeoutError(NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
    assertIsUnknownError(NativeTesting::TESTING_RegistrationService_SubmitVerificationErrorConvert);
  }

  private static <E extends Throwable> E assertRegistrationSessionErrorIs(
      String errorDescription, Class<E> expectedErrorType, ThrowingConsumer<String> throwError) {
    return assertThrows(
        "for " + errorDescription, expectedErrorType, () -> throwError.accept(errorDescription));
  }

  private static void assertIsRetryAfterError(ThrowingConsumer<String> throwError) {
    RetryLaterException e =
        assertRegistrationSessionErrorIs(
            "RetryAfter42Seconds", RetryLaterException.class, throwError);
    assertEquals(e.duration, Duration.ofSeconds(42));
  }

  private static void assertIsTimeoutError(ThrowingConsumer<String> throwError) {
    assertRegistrationSessionErrorIs("Timeout", ChatServiceException.class, throwError);
  }

  private static void assertIsUnknownError(ThrowingConsumer<String> throwError) {
    RegistrationException e =
        assertRegistrationSessionErrorIs("Unknown", RegistrationException.class, throwError);
    assertEquals(e.getMessage(), "some message");
  }

  @Test
  public void testFakeRemoteCreateSession() throws ExecutionException, InterruptedException {
    var tokio = new TokioAsyncContext();
    var serverAndCreateSession =
        RegistrationService.fakeCreateSession(
            tokio,
            new RegistrationService.CreateSession("+18005550123", "myPushToken", null, null));

    var fakeRemote = serverAndCreateSession.first().getNextRemote().get();
    var firstRequestAndId = fakeRemote.getNextIncomingRequest().get();
    assertNotNull(firstRequestAndId);
    var firstRequest = firstRequestAndId.first();

    assertEquals(firstRequest.getMethod(), "POST");
    assertEquals(firstRequest.getPathAndQuery(), "/v1/verification/session");

    fakeRemote.sendResponse(
        firstRequestAndId.second(),
        200,
        "OK",
        new String[] {"content-type: application/json"},
        """
        {
            "allowedToRequestCode": true,
            "verified": false,
            "requestedInformation": ["pushChallenge", "captcha"],
            "id": "fake-session-A"
        }
        """
            .getBytes());

    var session = serverAndCreateSession.second().get();
    assertEquals(session.getSessionId(), "fake-session-A");

    var sessionState = session.getSessionState();
    assertEquals(sessionState.getVerified(), false);
    assertEquals(
        sessionState.getRequestedInformation(),
        Set.of(
            RegistrationSessionState.RequestedInformation.PUSH_CHALLENGE,
            RegistrationSessionState.RequestedInformation.CAPTCHA));

    var requestVerification =
        session.requestVerificationCode(
            RegistrationService.VerificationTransport.VOICE,
            "libsignal test",
            Locale.CANADA_FRENCH);

    var secondRequestAndId = fakeRemote.getNextIncomingRequest().get();
    assertNotNull(secondRequestAndId);
    var secondRequest = secondRequestAndId.first();

    assertEquals(secondRequest.getMethod(), "POST");
    assertEquals(secondRequest.getPathAndQuery(), "/v1/verification/session/fake-session-A/code");
    assertEquals(
        new String(secondRequest.getBody()),
        """
        {"transport":"voice","client":"libsignal test"}""");
    assertEquals(
        secondRequest.getHeaders(),
        Map.of("content-type", "application/json", "accept-language", "fr-CA"));

    fakeRemote.sendResponse(
        secondRequestAndId.second(),
        200,
        "OK",
        new String[] {"content-type: application/json"},
        """
        {
            "allowedToRequestCode": true,
            "verified": false,
            "requestedInformation": ["pushChallenge", "captcha"],
            "id": "fake-session-A"
        }
        """
            .getBytes());

    requestVerification.get();
  }
}
