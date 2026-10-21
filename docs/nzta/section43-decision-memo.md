# Section 43 decision memo: whether to apply

Written for: the owner. This memo sets out the options. It does not recommend an option, and it does not recommend filing an application.

Decision owner: the owner of tpt-ruc-nz. The owner decides which option to take, who makes the application, and when.

## The question

Should TPT Solutions apply to NZTA for approval of a distance recorder under Section 43, for this product or for a version of it?

## Facts that bear on the choice

- The project is open source and dual licensed. Approval would apply to a specific device and software version, not to the licence.
- The software is a reference implementation. It is not approved, and its disclaimers say so.
- No hardware has been built. The GPS firmware's UART reader has never run on a board.
- There is no accuracy evidence yet. The test protocol is in `accuracy-test-protocol.md`.
- The approval criteria have not been read from NZTA's own text. See `section43-gap-analysis.md`.
- Approval would likely need an applicant with a legal identity, money for testing and for ongoing compliance, and someone to answer for the device over time.

Facts that need NZTA source: the application process, fees, timing, who may test, and whether software alone can be approved. Do not decide on the basis of guesses about these.

## Options

### Option A: Do not apply

Keep the product as a personal tracking and estimation tool, with its disclaimers. Do not market it for RUC licensing or tax deductions.

- **For:** no cost or compliance burden. Keeps the open-source reference role clear.
- **Against:** the product cannot be used for official licensing or claiming deductions. Users who need that will have to use another system.
- **Needs:** clear marketing and disclaimers, and no claims of approval anywhere.

### Option B: Apply for approval of this product

Build the device to the criteria, gather the test evidence, and apply.

- **For:** the only route to official use of this design.
- **Against:** the largest cost and effort. The gap analysis lists blocking gaps, including manual entry, tamper evidence and accuracy evidence. Approval may need changes that cut against the open-source design.
- **Needs:** the NZTA criteria read in full, a budget, an applicant entity, testing, and a long-term owner for the device.

### Option C: Partner or licence

Work with an organisation that already has an approved distance recorder, or that can apply on its own behalf, and provide the software or data for it.

- **For:** shares the approval cost and risk. Lets the project's software reach users who need approval.
- **Against:** depends on a partner's approval and terms. The licence and open-source model must be checked with the partner.
- **Needs:** a list of approved recorders and their terms, from NZTA or the vendors. This is not in the repository.

### Option D: Defer, and gather evidence first

Build the bench rig and run the accuracy protocol and the bench tests. Read the NZTA criteria. Decide later, with the results in hand.

- **For:** keeps the options open at low cost. The evidence is needed for any of the other options.
- **Against:** delays any decision. Some work may not be needed if the owner chooses Option A.
- **Needs:** the bench hardware, the test time, and the NZTA text.

## Questions the owner must answer

1. Is official RUC use of this product a goal, or only personal tracking?
2. Who would be the applicant, and do they have a legal identity to apply?
3. What budget is available for testing and for compliance over several years?
4. Would the owner accept changes to the open-source design, such as a sealed device, if approval required them?
5. Who would answer for the device after approval?

## Dependencies

- Read the NZTA text for every criterion, and record the source.
- Complete the bench tests before any decision on Option B.

## Record

| Date | Decision | Decided by |
|---|---|---|
| | Not yet decided | |
