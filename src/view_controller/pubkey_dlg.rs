// Bitcoin Pro: Professional bitcoin accounts & assets management
// Written in 2020-2022 by
//     Dr. Maxim Orlovsky <orlovsky@pandoraprime.ch>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![allow(clippy::needless_borrow)] // TODO: Remove unce bug in amplify_derive is fixed

use gtk::prelude::*;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::secp256k1;
use bitcoin::util::bip32::{
    self, ChildNumber, DerivationPath, ExtendedPrivKey, ExtendedPubKey,
};
use bitcoin::util::key;
use gtk::gdk;
use lnpbp::chain::Chain;
use miniscript::descriptor::DescriptorSinglePub;
use wallet::hd::{HardenedNormalSplit, SegmentIndexes};
use wallet::slip132::{self, FromSlip132};

use crate::model::TrackingAccount;

static UI: &str = include_str!("../view/pubkey.glade");

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
/// Errors from processing public key derivation data
pub enum Error {
    /// Wrong public key data
    #[display("{0}")]
    #[from]
    Secp(secp256k1::Error),

    /// Wrong public key data
    #[display("{0}")]
    #[from]
    Key(key::Error),

    /// BIP32-specific error
    #[display("{0}")]
    #[from]
    Bip32(bip32::Error),

    /// SLIP-32 specific error
    #[display("{0}")]
    #[from]
    Slip32(slip132::Error),

    /// Index range must not be empty
    RangeNotSpecified,

    /// Unable to parse '{0}' as index at position {1}
    WrongIndexNumber(String, usize),

    /// Unable to parse '{0}' as range at position {1}
    WrongRange(String, usize),

    /// Empty range specifier at position {0}
    EmptyRange(usize),

    /// Unsupported blockchain
    UnsupportedBlockchain,

    /// You need to specify blockchain type
    UnspecifiedBlockchain,

    /// You must provide a non-empty name
    EmptyName,

    /// For hardened derivation path you have to provide either account
    /// extended pubkey or master private key (not recommended)
    AccountXpubNeeded,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Display)]
#[display(Debug)]
pub enum PkType {
    Single,
    Hd,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Display)]
#[display(Debug)]
pub enum DeriveType {
    Bip44,
    Custom,
}

pub struct PubkeyDlg {
    dialog: gtk::Dialog,
    msg_box: gtk::Box,
    msg_label: gtk::Label,
    msg_image: gtk::Image,
    save_btn: gtk::Button,
    cancel_btn: gtk::Button,

    name_field: gtk::Entry,
    pubkey_field: gtk::Entry,
    xpub_field: gtk::Entry,
    account_field: gtk::Entry,

    sk_radio: gtk::RadioButton,
    hd_radio: gtk::RadioButton,

    bip44_radio: gtk::RadioButton,
    custom_radio: gtk::RadioButton,

    purpose_combo: gtk::ComboBox,
    purpose_index: gtk::SpinButton,
    purpose_chk: gtk::CheckButton,

    asset_combo: gtk::ComboBox,
    asset_index: gtk::SpinButton,
    asset_chk: gtk::CheckButton,

    account_index: gtk::SpinButton,
    account_chk: gtk::CheckButton,

    change_combo: gtk::ComboBox,
    change_index: gtk::SpinButton,
    change_chk: gtk::CheckButton,

    range_chk: gtk::CheckButton,
    range_field: gtk::Entry,
    derivation_field: gtk::Entry,

    network_combo: gtk::ComboBox,
    offset_index: gtk::SpinButton,
    offset_chk: gtk::CheckButton,
    offset_adj: gtk::Adjustment,

    xpubid_display: gtk::Entry,
    fingerprint_display: gtk::Entry,
    derivation_display: gtk::Entry,
    descriptor_display: gtk::Entry,
    xpub_display: gtk::Entry,

    uncompressed_display: gtk::Entry,
    compressed_display: gtk::Entry,
    xcoordonly_display: gtk::Entry,

    pkh_display: gtk::Entry,
    wpkh_display: gtk::Entry,
    wpkh_sh_display: gtk::Entry,
    taproot_display: gtk::Entry,
}

impl PubkeyDlg {
    pub fn load_glade() -> Option<Rc<Self>> {
        let builder = gtk::Builder::from_string(UI);

        let save_btn = builder.object("save")?;
        let cancel_btn = builder.object("cancel")?;

        let msg_box = builder.object("messageBox")?;
        let msg_image = builder.object("messageImage")?;
        let msg_label = builder.object("messageLabel")?;

        let name_field = builder.object("nameField")?;
        let pubkey_field = builder.object("pubkeyField")?;
        let xpub_field = builder.object("xpubField")?;
        let account_field = builder.object("accountField")?;
        let sk_radio = builder.object("singleKey")?;
        let hd_radio = builder.object("hdKey")?;
        let bip44_radio = builder.object("deriveBip44")?;
        let custom_radio = builder.object("deriveCustom")?;

        let purpose_combo = builder.object("purposeCombo")?;
        let purpose_index = builder.object("purposeCounter")?;
        let purpose_chk = builder.object("purposeCheck")?;

        let asset_combo = builder.object("assetCombo")?;
        let asset_index = builder.object("assetCounter")?;
        let asset_chk = builder.object("assetCheck")?;

        let account_index = builder.object("accountCounter")?;
        let account_chk = builder.object("accountCheck")?;

        let change_combo = builder.object("changeCombo")?;
        let change_index = builder.object("changeCounter")?;
        let change_chk = builder.object("changeCheck")?;

        let range_chk = builder.object("rangeCheck")?;
        let range_field = builder.object("rangeField")?;
        let derivation_field = builder.object("derivationField")?;

        let network_combo = builder.object("blockchainCombo")?;
        let offset_index = builder.object("exportIndex")?;
        let offset_chk = builder.object("exportHCheck")?;
        let offset_adj = builder.object("adjExport")?;

        let xpubid_display = builder.object("xpubidDisplay")?;
        let fingerprint_display = builder.object("fingerprintDisplay")?;
        let derivation_display = builder.object("derivationDisplay")?;
        let descriptor_display = builder.object("descriptorDisplay")?;
        let xpub_display = builder.object("xpubDisplay")?;

        let uncompressed_display = builder.object("uncompressedDisplay")?;
        let compressed_display = builder.object("compressedDisplay")?;
        let xcoordonly_display = builder.object("xonlyDisplay")?;

        let pkh_display = builder.object("pkhDisplay")?;
        let wpkh_display = builder.object("wpkhDisplay")?;
        let wpkh_sh_display = builder.object("wpkhShDisplay")?;
        let taproot_display = builder.object("taprootDisplay")?;

        let me = Rc::new(Self {
            dialog: glade_load!(builder, "pubkeyDlg").ok()?,
            save_btn,
            cancel_btn,
            msg_box,
            msg_image,
            msg_label,
            name_field,
            pubkey_field,
            xpub_field,
            account_field,
            sk_radio,
            hd_radio,
            bip44_radio,
            custom_radio,
            purpose_combo,
            purpose_index,
            purpose_chk,
            asset_combo,
            asset_index,
            asset_chk,
            account_index,
            account_chk,
            change_combo,
            change_index,
            change_chk,
            range_chk,
            range_field,
            derivation_field,
            network_combo,
            offset_index,
            offset_chk,
            offset_adj,
            xpubid_display,
            fingerprint_display,
            derivation_display,
            descriptor_display,
            xpub_display,
            uncompressed_display,
            compressed_display,
            xcoordonly_display,
            pkh_display,
            wpkh_display,
            wpkh_sh_display,
            taproot_display,
        });

        me.name_field.connect_changed(clone!(@weak me => move |_| {
            me.update_ui();
        }));

        me.pubkey_field
            .connect_changed(clone!(@weak me => move |_| {
                me.set_key_type(PkType::Single)
            }));

        me.range_field.connect_changed(clone!(@weak me => move |_| {
            me.set_key_type(PkType::Hd)
        }));

        me.range_chk.connect_toggled(clone!(@weak me => move |_| {
            if me.range_chk.is_active() && me.range_field.text().is_empty() {
                me.range_field.set_text(&format!("0-{}", u32::MAX));
            }
            me.set_key_type(PkType::Hd)
        }));

        for ctl in &[&me.xpub_field, &me.range_field, &me.account_field] {
            ctl.connect_changed(clone!(@weak me => move |_| {
                me.set_key_type(PkType::Hd)
            }));
        }

        me.derivation_field
            .connect_changed(clone!(@weak me => move |_| {
                me.set_derive_type(DeriveType::Custom)
            }));

        for ctl in &[
            &me.sk_radio,
            &me.hd_radio,
            &me.bip44_radio,
            &me.custom_radio,
        ] {
            ctl.connect_toggled(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        for ctl in &[
            &me.purpose_combo,
            &me.asset_combo,
            &me.change_combo,
            &me.network_combo,
        ] {
            ctl.connect_changed(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        for ctl in &[
            &me.purpose_index,
            &me.asset_index,
            &me.account_index,
            &me.change_index,
        ] {
            ctl.connect_changed(clone!(@weak me => move |_| {
                me.set_derive_type(DeriveType::Bip44)
            }));
        }

        for ctl in &[
            &me.purpose_chk,
            &me.asset_chk,
            &me.account_chk,
            &me.change_chk,
        ] {
            ctl.connect_toggled(clone!(@weak me => move |_| {
                me.set_derive_type(DeriveType::Bip44)
            }));
        }

        me.offset_index
            .connect_changed(clone!(@weak me => move |_| {
                me.update_ui();
            }));

        me.offset_chk.connect_toggled(clone!(@weak me => move |_| {
            me.update_ui();
        }));

        for ctl in &[
            &me.xpubid_display,
            &me.fingerprint_display,
            &me.derivation_display,
            &me.descriptor_display,
            &me.xpub_display,
            &me.uncompressed_display,
            &me.compressed_display,
            &me.xcoordonly_display,
            &me.pkh_display,
            &me.wpkh_display,
            &me.wpkh_sh_display,
            &me.taproot_display,
        ] {
            ctl.connect_icon_press(clone!(@weak ctl, @weak me => move |_, _, _| {
                let val = ctl.text();
                gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD)
                    .set_text(&val);
                me.display_info(format!("Value {} copied to clipboard", val));
            }));
        }

        Some(me)
    }
}

impl PubkeyDlg {
    pub fn run(
        self: Rc<Self>,
        tracking_account: Option<TrackingAccount>,
        chain: &Chain,
        on_save: impl Fn(TrackingAccount) + 'static,
        on_cancel: impl Fn() + 'static,
    ) {
        let me = self.clone();

        if let Some(tracking_account) = tracking_account {
            self.apply_tracking_account(tracking_account);
        }

        me.network_combo.set_active_id(Some(&chain.to_string()));

        me.update_ui();

        me.cancel_btn
            .connect_clicked(clone!(@weak self as me => move |_| {
                me.dialog.close();
                on_cancel()
            }));

        me.save_btn.connect_clicked(
            clone!(@weak self as me => move |_| match self.tracking_account() {
                Ok(tracking_account) => {
                    me.dialog.close();
                    on_save(tracking_account);
                }
                Err(err) => {
                    me.display_error(err);
                    me.save_btn.set_sensitive(false);
                }
            }),
        );

        me.dialog.run();
        me.dialog.close();
    }

    pub fn apply_tracking_account(&self, tracking_account: TrackingAccount) {
        self.name_field.set_text(&tracking_account.name);
        match tracking_account.key {
            descriptor::SingleSig::Pubkey(_) => {
                self.set_key_type(PkType::Single);
                self.pubkey_field
                    .set_text(&tracking_account.key.to_string());
            }
            descriptor::SingleSig::XPubDerivable(keyset) => {
                self.set_key_type(PkType::Hd);
                self.xpub_field.set_text(&keyset.master_xpub.to_string());
                self.account_field.set_text(&keyset.branch_xpub.to_string());
                self.range_field.set_text(&keyset.index_ranges_string());
                self.range_chk.set_active(keyset.index_ranges.is_some());

                // TODO: Parse BIP44 derivation info
                // let derivation: Vec<_> = keyset.derivation_path().into();
                // if derivation.len() == 6 {
                //     self.set_derive_type(DeriveType::Bip44);
                // } else {
                self.set_derive_type(DeriveType::Custom);
                self.derivation_field
                    .set_text(&keyset.derivation_path().to_string());
                // }
            }
            _ => unreachable!(),
        }
    }

    pub fn tracking_account(&self) -> Result<TrackingAccount, Error> {
        let key = if self.sk_radio.is_active() {
            descriptor::SingleSig::Pubkey(DescriptorSinglePub {
                origin: None,
                key: bitcoin::PublicKey::from_str(&self.pubkey_field.text())?,
            })
        } else {
            descriptor::SingleSig::XPubDerivable(self.derivation_components()?)
        };

        Ok(TrackingAccount {
            name: self.name_field.text().to_string(),
            key,
        })
    }

    pub fn derivation_path(
        &self,
        extended: bool,
    ) -> Result<DerivationPath, Error> {
        let mut derivation = if self.bip44_radio.is_active() {
            DerivationPath::from_str(&format!(
                "m/{}{}/{}{}/{}{}/{}{}",
                self.purpose_index.value() as u32,
                if self.purpose_chk.is_active() {
                    "'"
                } else {
                    ""
                },
                self.asset_index.value() as u32,
                if self.asset_chk.is_active() { "'" } else { "" },
                self.account_index.value() as u32,
                if self.account_chk.is_active() {
                    "'"
                } else {
                    ""
                },
                self.change_index.value() as u32,
                if self.change_chk.is_active() { "'" } else { "" }
            ))?
        } else {
            DerivationPath::from_str(&self.derivation_field.text())?
        };

        if extended {
            derivation = derivation.into_child(self.derivation_export_offset());
        }

        Ok(derivation)
    }

    pub fn derivation_export_offset(&self) -> ChildNumber {
        let index = self.offset_index.value() as u32;
        if self.offset_chk.is_active() {
            ChildNumber::Hardened { index }
        } else {
            ChildNumber::Normal { index }
        }
    }

    pub fn derivation_components(&self) -> Result<DerivationComponents, Error> {
        let derivation = self.derivation_path(false)?;
        let (branch_path, terminal_path) = derivation.hardened_normal_split();
        let account_xpub =
            ExtendedPubKey::from_slip132_str(&self.account_field.text());
        let master_xpub =
            ExtendedPubKey::from_slip132_str(&self.xpub_field.text());
        let index_ranges = self.derivation_ranges()?;

        if let Ok(master_priv) =
            ExtendedPrivKey::from_slip132_str(&self.xpub_field.text())
        {
            let master_xpub =
                ExtendedPubKey::from_priv(&secp256k1::SECP256K1, &master_priv);
            let branch_xpriv = master_priv
                .derive_priv(&secp256k1::SECP256K1, branch_path.as_ref())?;
            let branch_xpub =
                ExtendedPubKey::from_priv(&secp256k1::SECP256K1, &branch_xpriv);
            Ok(DerivationComponents {
                master_xpub,
                branch_xpub,
                branch_path,
                terminal_path,
                index_ranges,
            })
        } else if branch_path.as_ref().is_empty() {
            Ok(DerivationComponents {
                master_xpub: master_xpub.clone()?,
                branch_xpub: master_xpub?,
                branch_path,
                terminal_path,
                index_ranges,
            })
        } else if !self.account_field.text().is_empty() {
            Ok(DerivationComponents {
                master_xpub: master_xpub?,
                branch_path,
                branch_xpub: account_xpub?,
                terminal_path,
                index_ranges,
            })
        } else {
            Err(Error::AccountXpubNeeded)
        }
    }

    pub fn derivation_ranges(
        &self,
    ) -> Result<Option<DerivationRangeVec>, Error> {
        if !self.range_chk.is_active() {
            return Ok(None);
        }
        let mut index_ranges = vec![];
        for (pos, elem) in
            self.range_field.text().as_str().split(',').enumerate()
        {
            let mut split = elem.trim().split('-');
            let range = match (split.next(), split.next(), split.next()) {
                (None, None, None) => return Err(Error::EmptyRange(pos)),
                (Some(num), None, None) => {
                    let idx = num.parse().map_err(|_| {
                        Error::WrongIndexNumber(num.to_string(), pos)
                    })?;
                    RangeInclusive::new(idx, idx).into()
                }
                (Some(num1), Some(num2), None) => RangeInclusive::new(
                    num1.parse().map_err(|_| {
                        Error::WrongIndexNumber(num1.to_string(), pos)
                    })?,
                    num2.parse().map_err(|_| {
                        Error::WrongIndexNumber(num2.to_string(), pos)
                    })?,
                )
                .into(),
                _ => return Err(Error::WrongRange(elem.to_string(), pos)),
            };
            index_ranges.push(range);
        }
        DerivationRangeVec::try_from(index_ranges)
            .map(Option::Some)
            .map_err(|_| Error::RangeNotSpecified)
    }

    pub fn display_info(&self, msg: impl ToString) {
        self.msg_label.set_text(&msg.to_string());
        self.msg_image.set_from_icon_name(
            Some("dialog-information"),
            gtk::IconSize::SmallToolbar,
        );
        self.msg_box.set_visible(true);
    }

    pub fn display_error(&self, msg: impl std::error::Error) {
        self.msg_label.set_text(&msg.to_string());
        self.msg_image.set_from_icon_name(
            Some("dialog-error"),
            gtk::IconSize::SmallToolbar,
        );
        self.msg_box.set_visible(true);
    }

    pub fn set_key_type(&self, pk_type: PkType) {
        self.sk_radio.set_active(pk_type == PkType::Single);
        self.hd_radio.set_active(pk_type == PkType::Hd);
        self.update_ui();
    }

    pub fn set_derive_type(&self, derive_type: DeriveType) {
        self.bip44_radio
            .set_active(derive_type == DeriveType::Bip44);
        self.custom_radio
            .set_active(derive_type == DeriveType::Custom);
        self.update_ui();
    }

    pub fn update_ui(&self) {
        self.pubkey_field.set_sensitive(self.sk_radio.is_active());
        self.xpub_field.set_sensitive(self.hd_radio.is_active());
        self.account_field.set_sensitive(self.hd_radio.is_active());
        self.derivation_field
            .set_sensitive(self.custom_radio.is_active());
        self.range_field.set_sensitive(self.range_chk.is_active());
        self.range_chk.set_sensitive(self.hd_radio.is_active());

        self.offset_index.set_sensitive(self.hd_radio.is_active());
        self.offset_chk.set_sensitive(self.hd_radio.is_active());

        for ctl in &[&self.bip44_radio, &self.custom_radio] {
            ctl.set_sensitive(self.hd_radio.is_active());
        }

        for ctl in &[&self.purpose_combo, &self.asset_combo, &self.change_combo]
        {
            ctl.set_sensitive(
                self.hd_radio.is_active() && self.bip44_radio.is_active(),
            );
        }

        for ctl in &[
            &self.purpose_index,
            &self.asset_index,
            &self.account_index,
            &self.change_index,
        ] {
            ctl.set_sensitive(
                self.hd_radio.is_active() && self.bip44_radio.is_active(),
            );
        }

        for ctl in &[
            &self.purpose_chk,
            &self.asset_chk,
            &self.account_chk,
            &self.change_chk,
        ] {
            ctl.set_sensitive(
                self.hd_radio.is_active() && self.bip44_radio.is_active(),
            );
        }

        if self.purpose_combo.active() != Some(4) {
            self.purpose_index.set_sensitive(false);
            self.purpose_chk.set_sensitive(false);
            self.purpose_index.set_value(
                self.purpose_combo
                    .active_id()
                    .map(|s| f64::from_str(&s).unwrap_or_default())
                    .unwrap_or_default(),
            );
            self.purpose_chk.set_active(true);
        }

        if self.asset_combo.active() != Some(4) {
            self.asset_index.set_sensitive(false);
            self.asset_chk.set_sensitive(false);
            self.asset_index.set_value(
                self.asset_combo
                    .active_id()
                    .map(|s| f64::from_str(&s).unwrap_or_default())
                    .unwrap_or_default(),
            );
            self.asset_chk.set_active(true);
        }

        if self.change_combo.active() != Some(2) {
            self.change_index.set_sensitive(false);
            self.change_chk.set_sensitive(false);
            self.change_index.set_value(
                self.change_combo
                    .active_id()
                    .map(|s| f64::from_str(&s).unwrap_or_default())
                    .unwrap_or_default(),
            );
            self.change_chk.set_active(false);
        }

        match self.update_ui_internal() {
            Ok(None) => {
                self.msg_box.set_visible(false);
                self.save_btn.set_sensitive(true);
            }
            Ok(Some(msg)) => {
                self.display_info(msg);
                self.save_btn.set_sensitive(true);
            }
            Err(err) => {
                self.display_error(err);
                self.save_btn.set_sensitive(false);
            }
        }
    }

    pub fn update_ui_internal(&self) -> Result<Option<String>, Error> {
        let mut info_msg = None;

        let network = match self.network_combo.active() {
            Some(0) => bitcoin::Network::Bitcoin,
            Some(1) => bitcoin::Network::Testnet,
            Some(2) => bitcoin::Network::Testnet,
            None => return Err(Error::UnspecifiedBlockchain),
            _ => return Err(Error::UnsupportedBlockchain),
        };

        let pk = if self.sk_radio.is_active() {
            let pk_str = self.pubkey_field.text();
            bitcoin::PublicKey::from_str(&pk_str)?
        } else {
            self.offset_chk.set_sensitive(true);

            let derivation = self.derivation_path(true)?;
            let terminal = derivation
                .hardened_normal_split()
                .1
                .into_iter()
                .map(|index| ChildNumber::Normal {
                    index: index.first_index(),
                })
                .collect::<DerivationPath>();

            let (xpubkey, master) = if let Ok(master_priv) =
                ExtendedPrivKey::from_slip132_str(&self.xpub_field.text())
            {
                let master = ExtendedPubKey::from_priv(
                    &secp256k1::SECP256K1,
                    &master_priv,
                );
                self.account_field.set_sensitive(false);
                let prv = master_priv
                    .derive_priv(&secp256k1::SECP256K1, &derivation)?;
                (
                    ExtendedPubKey::from_priv(&secp256k1::SECP256K1, &prv),
                    master,
                )
            } else {
                let master =
                    ExtendedPubKey::from_slip132_str(&self.xpub_field.text())?;
                let pk = master
                    .derive_pub(&secp256k1::SECP256K1, &derivation)
                    .map(|pk| {
                        self.account_field.set_sensitive(false);
                        pk
                    })
                    .or_else(|_| -> Result<ExtendedPubKey, Error> {
                        self.account_field.set_sensitive(true);
                        if !self.account_field.text().is_empty() {
                            self.offset_chk.set_sensitive(false);
                            self.offset_chk.set_active(false);
                            let account = ExtendedPubKey::from_slip132_str(
                                &self.account_field.text(),
                            )?;
                            let pk = account.derive_pub(
                                &secp256k1::SECP256K1,
                                &terminal,
                            )?;
                            info_msg = Some(s!(
                                "NB: It is technically impossible to verify that the account key \
                                matches extended master public key so use their association at your \
                                own risk"
                            ));
                            Ok(pk)
                        } else {
                            Err(Error::AccountXpubNeeded)
                        }
                    })?;
                (pk, master)
            };

            self.xpubid_display
                .set_text(&xpubkey.identifier().to_string());
            self.fingerprint_display
                .set_text(&xpubkey.fingerprint().to_string());
            self.derivation_display.set_text(
                terminal
                    .to_string()
                    .strip_prefix("m/")
                    .expect("Derivation path always has this prefix"),
            );
            self.descriptor_display.set_text(&format!(
                "[{}]{}",
                master.fingerprint(),
                derivation
                    .to_string()
                    .strip_prefix('m')
                    .unwrap_or(&derivation.to_string())
            ));
            self.xpub_display.set_text(&xpubkey.to_string());

            if self.range_chk.is_active() {
                let (lower, upper) =
                    if let Some(ranges) = self.derivation_ranges()? {
                        (ranges.first_index(), ranges.last_index())
                    } else {
                        (0, wallet::hd::HARDENED_INDEX_BOUNDARY)
                    };
                self.offset_adj.set_lower(lower as f64);
                self.offset_adj.set_upper(upper as f64);

                if lower > self.offset_index.value_as_int() as u32 {
                    self.offset_index.set_value(lower as f64);
                }
                if upper < self.offset_index.value_as_int() as u32 {
                    self.offset_index.set_value(upper as f64);
                }
            }

            xpubkey.public_key
        };

        self.uncompressed_display.set_text(
            &bitcoin::PublicKey {
                compressed: false,
                inner: pk.key,
            }
            .to_string(),
        );

        let pkc = bitcoin::PublicKey {
            compressed: true,
            inner: pk.key,
        };
        self.compressed_display.set_text(&pkc.to_string());
        self.xcoordonly_display.set_text("Not yet supported");

        self.pkh_display
            .set_text(&bitcoin::Address::p2pkh(&pk, network).to_string());
        self.wpkh_display.set_text(
            &bitcoin::Address::p2wpkh(&pkc, network)
                .expect("The key is compressed")
                .to_string(),
        );
        self.wpkh_sh_display.set_text(
            &bitcoin::Address::p2shwpkh(&pkc, network)
                .expect("The key is compressed")
                .to_string(),
        );
        self.taproot_display.set_text("Not yet supported");

        if self.name_field.text().is_empty() {
            let err = Error::EmptyName;
            self.name_field.set_icon_from_icon_name(
                gtk::EntryIconPosition::Secondary,
                Some("dialog-error"),
            );
            self.name_field.set_icon_tooltip_text(
                gtk::EntryIconPosition::Secondary,
                Some(&err.to_string()),
            );
            return Err(err);
        } else {
            self.name_field.set_icon_from_icon_name(
                gtk::EntryIconPosition::Secondary,
                None,
            );
            self.name_field
                .set_icon_tooltip_text(gtk::EntryIconPosition::Secondary, None);
        }

        Ok(info_msg)
    }
}
