//
// Copyright 2023 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

package org.signal.libsignal.chat;

import com.google.protobuf.InvalidProtocolBufferException;
import org.signal.chat.profile.GetVersionedProfileRequest;
import org.signal.chat.profile.GetVersionedProfileResponse;
import org.signal.libsignal.internal.Native;
import org.signal.libsignal.internal.NativeHandleGuard;

public class ProfileClient implements NativeHandleGuard.Owner {

  private static final String DEFAULT_TARGET = "https://grpcproxy.gluonhq.net:443";

  private final long unsafeHandle;

  public ProfileClient() throws Exception {
    this(DEFAULT_TARGET);
  }

  public ProfileClient(String target) throws Exception {
    this.unsafeHandle = Native.ProfileClient_New(target);
  }

  @Override
  @SuppressWarnings("deprecation")
  protected void finalize() {
    Native.ProfileClient_Destroy(this.unsafeHandle);
  }

  public long unsafeNativeHandleWithoutGuard() {
    return this.unsafeHandle;
  }

  public GetVersionedProfileResponse getVersionedProfile(GetVersionedProfileRequest request)
      throws SignalChatCommunicationFailureException {
    try (NativeHandleGuard guard = new NativeHandleGuard(this)) {
      byte[] serializedResponse =
          Native.ProfileClient_GetVersionedProfile(guard.nativeHandle(), request.toByteArray());
      return GetVersionedProfileResponse.parseFrom(serializedResponse);
    } catch (Exception e) {
      throw new SignalChatCommunicationFailureException(e);
    }
  }
}
