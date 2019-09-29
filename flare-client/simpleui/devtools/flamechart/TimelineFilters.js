// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.


Flamechart.TimelineFilters = {};

Flamechart.TimelineFilters.IsLong = class extends TimelineModel.TimelineModelFilter {
  constructor() {
    super();
    this._minimumRecordDuration = 0;
  }

  /**
   * @param {number} value
   */
  setMinimumRecordDuration(value) {
    this._minimumRecordDuration = value;
  }

  /**
   * @override
   * @param {!SDK.TracingModel.Event} event
   * @return {boolean}
   */
  accept(event) {
    const duration = event.endTime ? event.endTime - event.startTime : 0;
    return duration >= this._minimumRecordDuration;
  }
};


Flamechart.TimelineFilters.Category = class extends TimelineModel.TimelineModelFilter {
  constructor() {
    super();
  }

  /**
   * @override
   * @param {!SDK.TracingModel.Event} event
   * @return {boolean}
   */
  accept(event) {
    return !Flamechart.TimelineUIUtils.eventStyle(event).category.hidden;
  }
};

Flamechart.TimelineFilters.RegExp = class extends TimelineModel.TimelineModelFilter {
  /**
   * @param {!RegExp=} regExp
   */
  constructor(regExp) {
    super();
    /** @type {?RegExp} */
    this._regExp;
    this.setRegExp(regExp || null);
  }

  /**
   * @param {?RegExp} regExp
   */
  setRegExp(regExp) {
    this._regExp = regExp;
  }

  /**
   * @return {?RegExp}
   */
  regExp() {
    return this._regExp;
  }

  /**
   * @override
   * @param {!SDK.TracingModel.Event} event
   * @return {boolean}
   */
  accept(event) {
    return !this._regExp || Flamechart.TimelineUIUtils.testContentMatching(event, this._regExp);
  }
};
