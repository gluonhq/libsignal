//
// Copyright 2023 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

package org.signal.libsignal.chat;

import com.google.protobuf.InvalidProtocolBufferException;
import org.signal.chat.device.GetDevicesRequest;
import org.signal.chat.device.GetDevicesResponse;
import org.signal.libsignal.internal.Native;
import org.signal.libsignal.internal.NativeHandleGuard;

public class DeviceClient implements NativeHandleGuard.Owner {

  private static final String DEFAULT_TARGET = "https://grpcproxy.gluonhq.net:443";

  private final long unsafeHandle;

  public DeviceClient() throws Exception {
    this(DEFAULT_TARGET);
  }

  public DeviceClient(String target) throws Exception {
    this.unsafeHandle = Native.DeviceClient_New(target);
  }

  @Override
  @SuppressWarnings("deprecation")
  protected void finalize() {
    Native.ProfileClient_Destroy(this.unsafeHandle);
  }

  public long unsafeNativeHandleWithoutGuard() {
    return this.unsafeHandle;
  }

  public GetDevicesResponse getDevices(GetDevicesRequest request, String authorization)
      throws SignalChatCommunicationFailureException {
    try (NativeHandleGuard guard = new NativeHandleGuard(this)) {
      byte[] serializedResponse =
          Native.DeviceClient_GetDevices(
              guard.nativeHandle(), request.toByteArray(), authorization);
      return GetDevicesResponse.parseFrom(serializedResponse);
    } catch (Exception e) {
      throw new SignalChatCommunicationFailureException(e);
    }
  }
}
