// Copyright 2019 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/**
 * @interface
 */
export default class App {
  /**
   * @param {!Document} document
   */
  presentUI(document) {
  }
}

/* Legacy exported object */
self.Common = self.Common || {};
Common = Common || {};

/**
 * @interface
 */
Common.App = App;
