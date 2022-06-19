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

use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::str::FromStr;

use wallet::descriptor::{self, ScriptConstruction, ScriptSource, Template};

use crate::controller::utxo_lookup::{self, UtxoLookup};
use crate::model::{
    DescriptorAccount, Document, ResolverError, TrackingAccount, UtxoEntry,
};
use crate::util::resolver_mode::{self, ResolverModeType};
use crate::view_controller::PubkeySelectDlg;

static UI: &str = include_str!("../view/descriptor.glade");

#[derive(Debug, Display, From, Error)]
#[display(doc_comments)]
/// Errors from processing descriptor data
pub enum Error {
    /// You must provide a non-empty name
    EmptyName,

    /// You must select key descriptor
    EmptyKey,

    /// You must use at least two unique keys in the multi-signature scheme
    EmptyKeyset,

    /// You must provide a non-empty script source
    EmptyScript,

    /// You need to specify type of the provided script
    SourceTypeRequired,

    /// {0} is not supported in the current version
    NotYetSupported(&'static str),

    /// You need to specify lookup method
    LookupTypeRequired,

    /// Unrecognizable lookup type string {0}
    #[from]
    LookupTypeUnrecognized(resolver_mode::ParseError),

    /// Error with Electrum server connection configuration
    #[display("{0}")]
    #[from]
    Resolver(ResolverError),

    /// Error during UTXO lookup operation
    #[display("{0}")]
    #[from]
    UtxoLookup(utxo_lookup::Error),
}

pub struct DescriptorDlg {
    dialog: gtk::Dialog,

    key: Rc<RefCell<Option<descriptor::SingleSig>>>,
    keyset: Rc<RefCell<Vec<descriptor::SingleSig>>>,
    utxo_set: Rc<RefCell<HashSet<UtxoEntry>>>,

    msg_box: gtk::Box,
    msg_label: gtk::Label,
    msg_image: gtk::Image,

    name_entry: gtk::Entry,

    singlesig_radio: gtk::RadioButton,
    multisig_radio: gtk::RadioButton,
    script_radio: gtk::RadioButton,

    singlesig_box: gtk::Box,
    pubkey_entry: gtk::Entry,
    multisig_frame: gtk::Frame,
    pubkey_tree: gtk::TreeView,
    pubkey_store: gtk::ListStore,
    threshold_spin: gtk::SpinButton,
    threshold_adj: gtk::Adjustment,
    script_frame: gtk::Frame,
    script_combo: gtk::ComboBox,
    script_text: gtk::TextView,
    script_buffer: gtk::TextBuffer,

    add_pk_btn: gtk::ToolButton,
    select_pk_btn: gtk::Button,
    insert_pk_btn: gtk::ToolButton,
    remove_pk_btn: gtk::ToolButton,

    bare_check: gtk::CheckButton,
    hash_check: gtk::CheckButton,
    compat_check: gtk::CheckButton,
    segwit_check: gtk::CheckButton,
    taproot_check: gtk::CheckButton,

    lookup_combo: gtk::ComboBox,
    lookup_btn: gtk::Button,
    utxo_tree: gtk::TreeView,
    utxo_store: gtk::ListStore,

    save_btn: gtk::Button,
    cancel_btn: gtk::Button,
}

impl DescriptorDlg {
    pub fn load_glade() -> Option<Rc<Self>> {
        let builder = gtk::Builder::from_string(UI);

        let save_btn = builder.get_object("save")?;
        let cancel_btn = builder.get_object("cancel")?;

        let msg_box = builder.get_object("messageBox")?;
        let msg_image = builder.get_object("messageImage")?;
        let msg_label = builder.get_object("messageLabel")?;

        let name_entry = builder.get_object("nameEntry")?;

        let singlesig_radio = builder.get_object("singlesigRadio")?;
        let singlesig_box = builder.get_object("singlesigBox")?;
        let pubkey_entry = builder.get_object("pubkeyEntry")?;

        let multisig_radio = builder.get_object("multisigRadio")?;
        let multisig_frame = builder.get_object("multisigFrame")?;
        let threshold_spin = builder.get_object("thresholdSpinner")?;
        let threshold_adj = builder.get_object("thresholdAdj")?;
        let pubkey_tree = builder.get_object("pubkeyTree")?;
        let pubkey_store = builder.get_object("pubkeyStore")?;

        let script_radio = builder.get_object("scriptRadio")?;
        let script_frame = builder.get_object("scriptFrame")?;
        let script_combo = builder.get_object("scriptCombo")?;
        let script_text = builder.get_object("scriptText")?;
        let script_buffer = builder.get_object("scriptBuffer")?;

        let select_pk_btn = builder.get_object("selectPubkey")?;
        let add_pk_btn = builder.get_object("addPubkey")?;
        let insert_pk_btn = builder.get_object("insertPubkey")?;
        let remove_pk_btn = builder.get_object("removePubkey")?;

        let bare_check = builder.get_object("bareChk")?;
        let hash_check = builder.get_object("hashChk")?;
        let compat_check = builder.get_object("compatChk")?;
        let segwit_check = builder.get_object("segwitChk")?;
        let taproot_check = builder.get_object("taprootChk")?;

        let lookup_combo = builder.get_object("lookupCombo")?;
        let lookup_btn = builder.get_object("lookupBtn")?;
        let utxo_tree = builder.get_object("utxoTree")?;
        let utxo_store = builder.get_object("utxoStore")?;

        let me = Rc::new(Self {
            dialog: glade_load!(builder, "descriptorDlg").ok()?,

            key: none!(),
            keyset: empty!(),
            utxo_set: empty!(),

            msg_box,
            msg_image,
            msg_label,

            name_entry,

            singlesig_radio,
            singlesig_box,
            multisig_radio,
            script_radio,
            pubkey_entry,
            multisig_frame,
            pubkey_tree,
            pubkey_store,
            threshold_spin,
            threshold_adj,
            script_frame,
            script_combo,
            script_text,
            script_buffer,

            add_pk_btn,
            select_pk_btn,
            insert_pk_btn,
            remove_pk_btn,

            bare_check,
            hash_check,
            compat_check,
            segwit_check,
            taproot_check,

            lookup_combo,
            lookup_btn,
            utxo_tree,
            utxo_store,

            save_btn,
            cancel_btn,
        });

        for ctl in &[&me.singlesig_radio, &me.multisig_radio, &me.script_radio]
        {
            ctl.connect_toggled(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        for ctl in &[
            &me.bare_check,
            &me.hash_check,
            &me.compat_check,
            &me.segwit_check,
            &me.taproot_check,
        ] {
            ctl.connect_toggled(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        for ctl in &[&me.name_entry, &me.pubkey_entry] {
            ctl.connect_changed(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        for ctl in &[&me.script_combo, &me.lookup_combo] {
            ctl.connect_changed(clone!(@weak me => move |_| {
                me.update_ui()
            }));
        }

        me.threshold_spin
            .connect_changed(clone!(@weak me => move |_| {
                me.update_ui()
            }));

        me.script_buffer
            .connect_changed(clone!(@weak me => move |_| {
                me.update_ui()
            }));

        Some(me)
    }
}

impl DescriptorDlg {
    pub fn run(
        self: Rc<Self>,
        doc: Rc<RefCell<Document>>,
        descriptor_generator: Option<DescriptorAccount>,
        on_save: impl Fn(DescriptorAccount, HashSet<UtxoEntry>) + 'static,
        on_cancel: impl Fn() + 'static,
    ) {
        let me = self.clone();

        if let Some(descriptor_generator) = descriptor_generator {
            self.apply_descriptor_generator(doc.clone(), descriptor_generator);
        }

        me.update_ui();

        me.select_pk_btn.connect_clicked(
            clone!(@weak me, @strong doc => move |_| {
                let pubkey_dlg = PubkeySelectDlg::load_glade().expect("Must load");
                pubkey_dlg.run(
                    doc.clone(),
                    clone!(@weak me => move |tracking_account| {
                        let key = tracking_account.key;
                        me.pubkey_entry.set_text(&key.to_string());
                        *me.key.borrow_mut() = Some(key);
                    }),
                    || {},
                );

                me.update_ui()
            }),
        );

        me.add_pk_btn.connect_clicked(
            clone!(@weak me, @strong doc => move |_| {
                let pubkey_dlg = PubkeySelectDlg::load_glade().expect("Must load");
                pubkey_dlg.run(
                    doc.clone(),
                    clone!(@weak me => move |tracking_account| {
                        me.pubkey_store.insert_with_values(None, &[0, 1, 2], &[
                            &tracking_account.name(),
                            &tracking_account.details(),
                            &tracking_account.count(),
                        ]);
                        me.keyset.borrow_mut().push(tracking_account.key);
                    }),
                    || {},
                );
                me.update_ui()
            }),
        );

        me.insert_pk_btn.connect_clicked(
            clone!(@weak me, @strong doc => move |_| {
                let pubkey_dlg = PubkeySelectDlg::load_glade().expect("Must load");
                pubkey_dlg.run(
                    doc.clone(),
                    clone!(@weak me => move |tracking_account| {
                        me.script_buffer.insert_at_cursor(&tracking_account.details());
                    }),
                    || {},
                );
                me.update_ui()
            }),
        );

        me.remove_pk_btn.connect_clicked(
            clone!(@weak me, @strong doc => move |_| {
                if let Some((model, iter)) =
                        me.pubkey_tree.get_selection().get_selected() {
                    let key = model
                        .get_value(&iter, 1)
                        .get::<String>()
                        .expect("Must always be parseble")
                        .expect("Key is always present");
                    if let Some(tracking_account) =
                            doc.borrow().tracking_account_by_key(&key) {
                        let pos = me.keyset
                            .borrow()
                            .iter()
                            .position(|k| k == &tracking_account.key)
                            .expect("Key was just found, so position is present");
                        me.keyset.borrow_mut().remove(pos);
                    }
                    me.pubkey_store.remove(&iter);
                }
                me.update_ui()
            }),
        );

        me.lookup_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            match me.descriptor_generator() {
                Ok(descriptor_account) => {
                    if let descriptor::Template::Scripted(..) = descriptor_account.generator.template {
                        me.display_error(Error::NotYetSupported("Custom script lookup"))
                    } else if let Err(err) = me.lookup(doc.clone(), descriptor_account) {
                        me.display_error(err);
                    }
                },
                Err(err) => {
                    me.display_error(err);
                    me.lookup_combo.set_sensitive(false);
                    me.lookup_btn.set_sensitive(false);
                }
            }
        }));

        me.cancel_btn.connect_clicked(clone!(@weak me => move |_| {
            me.dialog.close();
            on_cancel()
        }));

        me.save_btn.connect_clicked(
            clone!(@weak me => move |_| match self.descriptor_generator() {
                Ok(descriptor_generator) => {
                    me.dialog.close();
                    let utxo_set = (*me.utxo_set).clone().into_inner();
                    on_save(descriptor_generator, utxo_set);
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

    pub fn apply_descriptor_generator(
        &self,
        doc: Rc<RefCell<Document>>,
        account: DescriptorAccount,
    ) {
        self.name_entry.set_text(&account.name);
        match account.generator.template {
            descriptor::Template::SingleSig(key) => {
                self.singlesig_radio.set_active(true);
                self.pubkey_entry.set_text(&key.to_string());
                *self.key.borrow_mut() = Some(key);
            }
            descriptor::Template::MultiSig(multisig) => {
                self.threshold_spin.set_value(multisig.threshold() as f64);
                let doc = doc.borrow();
                for key in multisig.pubkeys {
                    let tracking_account = doc
                        .tracking_account_by_key(&key.to_string())
                        .unwrap_or(TrackingAccount {
                            name: s!("<Unrecognized key>"),
                            key: key.clone(),
                        });
                    self.pubkey_store.insert_with_values(
                        None,
                        &[0, 1, 2],
                        &[
                            &tracking_account.name(),
                            &tracking_account.details(),
                            &tracking_account.count(),
                        ],
                    );
                    self.keyset.borrow_mut().push(key);
                }
            }
            descriptor::Template::Scripted(script_source) => {
                self.script_radio.set_active(true);
                self.script_combo.set_active_id(Some(
                    match script_source.script {
                        ScriptConstruction::ScriptTemplate(_) => "asm",
                        ScriptConstruction::Miniscript(_) => "miniscript",
                        ScriptConstruction::MiniscriptPolicy(_) => "policy",
                        _ => "asm",
                    },
                ));
                self.script_buffer.set_text(&script_source.to_string());
            }
            Template::MuSigBranched(_) => unimplemented!(),
            _ => unimplemented!(),
        }
        self.bare_check.set_active(account.generator.variants.bare);
        self.hash_check
            .set_active(account.generator.variants.hashed);
        self.compat_check
            .set_active(account.generator.variants.nested);
        self.segwit_check
            .set_active(account.generator.variants.segwit);
        self.taproot_check
            .set_active(account.generator.variants.taproot);
    }

    pub fn descriptor_generator(&self) -> Result<DescriptorAccount, Error> {
        let template = self.descriptor_content()?;
        let variants = self.descriptor_types();

        // TODO: Make sure that types are compatible with the content

        let name = self.name_entry.get_text().to_string();
        if name.is_empty() {
            return Err(Error::EmptyName);
        }
        Ok(DescriptorAccount {
            name,
            generator: descriptor::Generator { template, variants },
        })
    }

    pub fn descriptor_content(&self) -> Result<descriptor::Template, Error> {
        let content = if self.singlesig_radio.get_active() {
            let key = self.key.borrow().clone().ok_or(Error::EmptyKey)?;
            descriptor::Template::SingleSig(key)
        } else if self.multisig_radio.get_active() {
            let pubkeys = self.keyset.borrow().clone();
            if pubkeys.len() < 2 {
                return Err(Error::EmptyKeyset);
            }
            let threshold = Some(self.threshold_spin.get_value_as_int() as u8);
            descriptor::Template::MultiSig(descriptor::MultiSig {
                threshold,
                pubkeys,
                // TODO: Support modification of this flag with a UI
                reorder: true,
            })
        } else {
            let source = self
                .script_buffer
                .get_text(
                    &self.script_buffer.get_start_iter(),
                    &self.script_buffer.get_end_iter(),
                    false,
                )
                .ok_or(Error::EmptyScript)?
                .to_string();
            if source.is_empty() {
                return Err(Error::EmptyScript);
            }
            // TODO: Implement script parsing
            #[allow(unused_variables)]
            let script = match self
                .script_combo
                .get_active_id()
                .ok_or(Error::SourceTypeRequired)?
                .as_str()
            {
                "asm" => {
                    return Err(Error::NotYetSupported(
                        "Script parsing is not yet implemented",
                    ))
                }
                "hex" => {
                    return Err(Error::NotYetSupported(
                        "Script parsing is not yet implemented",
                    ))
                }
                "miniscript" => {
                    return Err(Error::NotYetSupported(
                        "Script parsing is not yet implemented",
                    ))
                }
                "policy" => {
                    return Err(Error::NotYetSupported(
                        "Script parsing is not yet implemented",
                    ))
                }
                _ => return Err(Error::SourceTypeRequired),
            };
            #[allow(unreachable_code)]
            descriptor::Template::Scripted(ScriptSource {
                script,
                source: Some(source),
                // TODO: Present an option of selecting tweak target via UI
                tweak_target: None,
            })
        };

        Ok(content)
    }

    pub fn descriptor_types(&self) -> descriptor::Variants {
        descriptor::Variants {
            bare: self.bare_check.get_active(),
            hashed: self.hash_check.get_active(),
            nested: self.compat_check.get_active(),
            segwit: self.segwit_check.get_active(),
            taproot: self.taproot_check.get_active(),
        }
    }

    pub fn lookup(
        &self,
        doc: Rc<RefCell<Document>>,
        generator: DescriptorAccount,
    ) -> Result<(), Error> {
        self.utxo_lookup(
            doc.borrow().resolver()?,
            ResolverModeType::from_str(
                &*self
                    .lookup_combo
                    .get_active_id()
                    .ok_or(Error::LookupTypeRequired)?,
            )?,
            generator,
            self.utxo_set.clone(),
            Some(&self.utxo_store),
        )?;

        Ok(())
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

    pub fn update_ui(&self) {
        let is_singlesig = self.singlesig_radio.get_active();
        let is_multisig = self.multisig_radio.get_active();
        let is_lockscript = self.script_radio.get_active();

        self.singlesig_box.set_sensitive(is_singlesig);
        self.multisig_frame.set_sensitive(is_multisig);
        self.threshold_spin.set_sensitive(is_multisig);
        self.script_frame.set_sensitive(is_lockscript);
        self.script_combo.set_sensitive(is_lockscript);

        self.threshold_adj
            .set_upper(self.keyset.borrow().len() as f64);

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
        self.lookup_btn.set_sensitive(false);
        self.lookup_combo.set_sensitive(false);

        let _ = self.descriptor_generator()?;

        self.lookup_btn.set_sensitive(true);
        self.lookup_combo.set_sensitive(true);

        Ok(None)
    }
}

impl UtxoLookup for DescriptorDlg {}
