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

use gtk::gdk;
use gtk::gdk_pixbuf::{InterpType, PixbufLoader};
use gtk::prelude::*;
use qrcode_generator::QrCodeEcc;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::{OutPoint, Txid};
use rgb::{Consignment, ContractId, ToBech32};
use rgb20::SupplyMeasure;

use crate::model::Document;
use crate::view_controller::{AssetDlg, DescriptorDlg, PubkeyDlg, SaveDlg};

static UI: &str = include_str!("../view/bpro.glade");

#[derive(Debug, Display, Error, From)]
#[display(doc_comments)]
pub enum Error {
    /// Glade error: {0}
    #[from]
    GladeError(crate::Error),

    /// Document-based error
    #[from]
    #[display("{0}")]
    Document(crate::model::Error),
}

pub struct BproWin {
    window: gtk::ApplicationWindow,
    pubkey_tree: gtk::TreeView,
    pubkey_store: gtk::ListStore,
    descriptor_tree: gtk::TreeView,
    descriptor_store: gtk::ListStore,
    utxo_descr_tree: gtk::TreeView,
    utxo_descr_store: gtk::ListStore,
    utxo_tree: gtk::TreeView,
    utxo_store: gtk::ListStore,
    asset_tree: gtk::TreeView,
    asset_store: gtk::ListStore,
    header_bar: gtk::HeaderBar,
    new_btn: gtk::Button,
    open_btn: gtk::Button,
    pubkey_edit_btn: gtk::ToolButton,
    pubkey_remove_btn: gtk::ToolButton,
    descriptor_edit_btn: gtk::ToolButton,
    descriptor_remove_btn: gtk::ToolButton,
    utxo_descr_remove_btn: gtk::ToolButton,
    utxo_descr_clear_btn: gtk::ToolButton,
    utxo_remove_btn: gtk::ToolButton,
    asset_remove_btn: gtk::ToolButton,
    asset_id_display: gtk::Entry,
    asset_genesis_display: gtk::Entry,
    asset_contract_display: gtk::TextBuffer,
    asset_issued_display: gtk::Entry,
    asset_total_display: gtk::Entry,
    asset_decimals_display: gtk::Entry,
    asset_qr_image: gtk::Image,
}

impl BproWin {
    fn load_glade(doc: Option<Document>) -> Option<Rc<RefCell<Self>>> {
        let mut needs_save = true;
        let doc = Rc::new(RefCell::new(if let Some(doc) = doc {
            needs_save = false;
            doc
        } else {
            Document::new()
        }));

        let builder = gtk::Builder::from_string(UI);

        let new_btn: gtk::Button = builder.object("new")?;
        let open_btn: gtk::Button = builder.object("open")?;
        let header_bar: gtk::HeaderBar = builder.object("headerBar")?;

        let pubkey_edit_btn = builder.object("pubkeyEdit")?;
        let pubkey_remove_btn = builder.object("pubkeyRemove")?;
        let descriptor_edit_btn = builder.object("descriptorEdit")?;
        let descriptor_remove_btn = builder.object("descriptorRemove")?;
        let utxo_descr_remove_btn = builder.object("utxoDescrRemove")?;
        let utxo_descr_clear_btn = builder.object("utxoDescrClear")?;
        let utxo_remove_btn = builder.object("utxoRemove")?;
        let asset_remove_btn = builder.object("assetRemove")?;

        let pubkey_tree = builder.object("pubkeyTree")?;
        let pubkey_store = builder.object("pubkeyStore")?;
        let descriptor_tree = builder.object("locatorTree")?;
        let descriptor_store = builder.object("locatorStore")?;
        let utxo_descr_tree = builder.object("utxoDescrTree")?;
        let utxo_descr_store = builder.object("utxoDescrStore")?;
        let utxo_tree = builder.object("utxoTree")?;
        let utxo_store = builder.object("utxoStore")?;
        let asset_tree = builder.object("assetTree")?;
        let asset_store = builder.object("assetStore")?;

        let asset_id_display = builder.object("assetIdDisplay")?;
        let asset_genesis_display = builder.object("assetGenesisDisplay")?;
        let asset_contract_display = builder.object("assetContractDisplay")?;
        let asset_issued_display = builder.object("assetIssuedDisplay")?;
        let asset_total_display = builder.object("assetTotalDisplay")?;
        let asset_decimals_display = builder.object("assetDecimalsDisplay")?;
        let asset_qr_image = builder.object("assetQR")?;

        let chain_combo: gtk::ComboBox = builder.object("chainCombo")?;
        let electrum_radio: gtk::RadioButton = builder.object("electrum")?;
        let electrum_field: gtk::Entry = builder.object("electrumField")?;
        let electrum_btn: gtk::Button = builder.object("electrumBtn")?;

        doc.borrow().fill_tracking_store(&pubkey_store);
        doc.borrow().fill_descriptor_store(&descriptor_store);
        doc.borrow().fill_utxo_store(&utxo_store, None);
        doc.borrow().fill_asset_store(&asset_store);

        header_bar.set_subtitle(Some(&doc.borrow().name()));

        chain_combo.set_active_id(Some(&doc.borrow().chain().to_string()));
        electrum_radio.set_active(true);
        electrum_field.set_text(&doc.borrow().electrum().unwrap_or_default());

        let me = Rc::new(RefCell::new(Self {
            window: glade_load!(builder, "appWindow").ok()?,
            pubkey_tree,
            pubkey_store,
            descriptor_tree,
            descriptor_store,
            utxo_descr_tree,
            utxo_descr_store,
            utxo_tree,
            utxo_store,
            asset_tree,
            asset_store,
            header_bar,
            new_btn,
            open_btn,
            pubkey_edit_btn,
            pubkey_remove_btn,
            descriptor_edit_btn,
            descriptor_remove_btn,
            utxo_descr_remove_btn,
            utxo_descr_clear_btn,
            utxo_remove_btn,
            asset_remove_btn,
            asset_id_display,
            asset_genesis_display,
            asset_contract_display,
            asset_issued_display,
            asset_total_display,
            asset_decimals_display,
            asset_qr_image,
        }));

        chain_combo.connect_changed(
            clone!(@weak chain_combo, @strong doc => move |_| {
                if let Some(chain_name) = chain_combo.active_id() {
                    let _ = doc.borrow_mut().set_chain(&chain_name);
                }
            }),
        );

        electrum_field.connect_changed(
            clone!(@strong doc, @weak electrum_field => move |_| {
                match electrum_field.text().to_string().parse() {
                    Ok(addr) => {
                        electrum_field.set_secondary_icon_name(None);
                        electrum_field.set_secondary_icon_tooltip_text(
                            Some("")
                        );
                        let _ = doc.borrow_mut().set_electrum(addr);
                    }
                    Err(err) => {
                        electrum_field.set_secondary_icon_name(
                            Some("dialog-error")
                        );
                        electrum_field.set_secondary_icon_tooltip_text(
                            Some(&err.to_string())
                        );
                    }
                }
            }),
        );

        electrum_btn.connect_clicked(
            clone!(@strong doc, @weak electrum_field => move |_| {
                if let Err(err) = doc.borrow().resolver() {
                    electrum_field.set_secondary_icon_name(
                        Some("dialog-error")
                    );
                    electrum_field.set_secondary_icon_tooltip_text(
                        Some(&err.to_string())
                    );
                } else {
                    electrum_field.set_secondary_icon_name(
                        Some("dialog-ok")
                    );
                    electrum_field.set_secondary_icon_tooltip_text(
                        Some("")
                    );
                }
            }),
        );

        me.borrow().pubkey_tree.selection().connect_changed(
            clone!(@weak me => move |_| {
                let me = me.borrow();
                if me.pubkey_selection().is_some() {
                    me.pubkey_edit_btn.set_sensitive(true);
                    me.pubkey_remove_btn.set_sensitive(true);
                } else {
                    me.pubkey_edit_btn.set_sensitive(false);
                    me.pubkey_remove_btn.set_sensitive(false);
                }
            }),
        );

        let tb: gtk::ToolButton = builder.object("pubkeyAdd")?;
        tb.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let pubkey_dlg = PubkeyDlg::load_glade().expect("Must load");
            let chain = doc.borrow().chain().clone();
            pubkey_dlg.run(None, &chain, clone!(@weak me, @strong doc =>
                move |tracking_account| {
                    let me = me.borrow();
                    me.pubkey_store.insert_with_values(
                        None,
                        &[(0, &tracking_account.name()), (1, &tracking_account.details()), (2, &tracking_account.count())]
                    );
                    let _ = doc.borrow_mut().add_tracking_account(tracking_account);
                }),
                || {},
            );
        }));

        me.borrow().pubkey_edit_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let meb = me.borrow();
            let pubkey_dlg = PubkeyDlg::load_glade().expect("Must load");
            if let Some((keyname, _, iter)) = meb.pubkey_selection() {
                let tracking_account = doc
                    .borrow()
                    .tracking_account_by_key(&keyname)
                    .expect("Tracking account must be known since it is selected");
                let chain = doc.borrow().chain().clone();
                pubkey_dlg.run(Some(tracking_account.clone()), &chain, clone!(@weak me, @strong doc =>
                    move |new_tracking_account| {
                        let me = me.borrow();
                        me.pubkey_store.set_value(&iter, 0, &new_tracking_account.name().to_value());
                        me.pubkey_store.set_value(&iter, 1, &new_tracking_account.details().to_value());
                        me.pubkey_store.set_value(&iter, 2, &new_tracking_account.count().to_value());
                        let _ = doc.borrow_mut().update_tracking_account(&tracking_account, new_tracking_account);
                    }),
                    || {},
                );
            }
        }));

        me.borrow().pubkey_remove_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let me = me.borrow();
            if let Some((keyname, _, iter)) = me.pubkey_selection() {
                let tracking_account = doc
                    .borrow()
                    .tracking_account_by_key(&keyname)
                    .expect("Tracking account must be known since it is selected");
                let dlg = gtk::MessageDialog::new(
                    Some(&me.window),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::YesNo,
                    &format!(
                        "Please confirm deletion of the public key tracking account for {}", 
                        tracking_account.key
                    )
                );
                if dlg.run() == gtk::ResponseType::Yes {
                    me.pubkey_store.remove(&iter);
                    let _ = doc.borrow_mut().remove_tracking_account(tracking_account);
                }
                dlg.hide();
            }
        }));

        me.borrow().descriptor_tree.selection().connect_changed(
            clone!(@weak me, @strong doc => move |_| {
                let me = me.borrow();
                me.utxo_descr_store.clear();
                if let Some((generator, _, _)) = me.descriptor_selection() {
                    if let Some(descriptor_generator) = doc.borrow().descriptor_by_generator(&generator) {
                        doc.borrow().fill_utxo_store(&me.utxo_descr_store, Some(&descriptor_generator));
                    }
                    me.descriptor_edit_btn.set_sensitive(true);
                    me.descriptor_remove_btn.set_sensitive(true);
                } else {
                    me.descriptor_edit_btn.set_sensitive(false);
                    me.descriptor_remove_btn.set_sensitive(false);
                }
                me.utxo_descr_clear_btn.set_sensitive(me.utxo_descr_store.iter_first().is_some());
            }),
        );

        let tb: gtk::ToolButton = builder.object("descriptorAdd")?;
        tb.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let descriptor_dlg = DescriptorDlg::load_glade().expect("Must load");
            descriptor_dlg.run(doc.clone(), None, clone!(@weak me, @strong doc =>
                move |descriptor_generator, utxo_set_update| {
                    let me = me.borrow();
                    me.descriptor_store.insert_with_values(
                        None,
                        &[
                            (0, &descriptor_generator.name()),
                            (1, &descriptor_generator.type_name()),
                            (2, &descriptor_generator.descriptor()),
                        ],
                    );
                    let _ = doc.borrow_mut().add_descriptor(descriptor_generator);
                    let _ = doc.borrow_mut().update_utxo_set(utxo_set_update);
                }),
                || {},
            );
        }));

        me.borrow().descriptor_edit_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let meb = me.borrow();
            let descriptor_dlg = DescriptorDlg::load_glade().expect("Must load");
            if let Some((generator, _, iter)) = meb.descriptor_selection() {
                let descriptor_generator = doc
                    .borrow()
                    .descriptor_by_generator(&generator)
                    .expect("Descriptor account must be known since it is selected");
                descriptor_dlg.run(doc.clone(), Some(descriptor_generator.clone()), clone!(@weak me, @strong doc =>
                    move |new_descriptor_generator, utxo_set_update| {
                        let me = me.borrow();
                        me.utxo_descr_clear_btn.set_sensitive(!utxo_set_update.is_empty());
                        me.descriptor_store.set_value(&iter, 0, &new_descriptor_generator.name().to_value());
                        me.descriptor_store.set_value(&iter, 1, &new_descriptor_generator.type_name().to_value());
                        me.descriptor_store.set_value(&iter, 2, &new_descriptor_generator.descriptor().to_value());
                        let _ = doc.borrow_mut().update_descriptor(&descriptor_generator, new_descriptor_generator);
                        let _ = doc.borrow_mut().update_utxo_set(utxo_set_update);
                        doc.borrow().fill_utxo_store(&me.utxo_descr_store, Some(&descriptor_generator));
                        doc.borrow().fill_utxo_store(&me.utxo_store, None);
                    }),
                    || {},
                );
            }
        }));

        me.borrow().descriptor_remove_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let me = me.borrow();
            if let Some((generator, _, iter)) = me.descriptor_selection() {
                let descriptor_generator = doc
                    .borrow()
                    .descriptor_by_generator(&generator)
                    .expect("Descriptor must be known since it is selected");
                let dlg = gtk::MessageDialog::new(
                    Some(&me.window),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::YesNo,
                    &format!(
                        "Please confirm deletion of the descriptor '{}' defined by {}",
                        descriptor_generator.name(),
                        descriptor_generator.descriptor()
                    )
                );
                if dlg.run() == gtk::ResponseType::Yes {
                    me.descriptor_store.remove(&iter);
                    let _ = doc.borrow_mut().remove_descriptor(descriptor_generator);
                }
                dlg.hide();
            }
        }));

        me.borrow().utxo_descr_tree.selection().connect_changed(
            clone!(@weak me => move |_| {
                let me = me.borrow();
                me.utxo_descr_remove_btn.set_sensitive(me.utxo_descr_tree.selection().selected().is_some());
            }),
        );

        me.borrow().utxo_descr_remove_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let me = me.borrow();
            if let Some((outpoint, _, iter)) = Self::utxo_selection(&me.utxo_descr_tree) {
                let utxo = doc
                    .borrow()
                    .utxo_by_outpoint(outpoint)
                    .expect("UTXO must be known since it is selected");
                let dlg = gtk::MessageDialog::new(
                    Some(&me.window),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::YesNo,
                    &format!("Please confirm deletion of {}", utxo)
                );
                if dlg.run() == gtk::ResponseType::Yes {
                    me.utxo_descr_store.remove(&iter);
                    let _ = doc.borrow_mut().remove_utxo(utxo);
                    doc.borrow().fill_utxo_store(&me.utxo_store, None);
                }
                dlg.hide();
            }
        }));

        me.borrow().utxo_descr_clear_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let me = me.borrow();
            if let Some((generator, _, _)) = me.descriptor_selection() {
                let descriptor_generator = doc
                    .borrow()
                    .descriptor_by_generator(&generator)
                    .expect("Descriptor must be known since it is selected");
                let dlg = gtk::MessageDialog::new(
                    Some(&me.window),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::YesNo,
                    &format!("Please confirm deletion of all UTXOs for {}", generator)
                );
                if dlg.run() == gtk::ResponseType::Yes {
                    me.utxo_descr_store.clear();
                    let _ = doc.borrow_mut().remove_utxo_by_descriptor(descriptor_generator);
                    doc.borrow().fill_utxo_store(&me.utxo_store, None);
                    me.utxo_descr_clear_btn.set_sensitive(false);
                }
                dlg.hide();
            }
        }));

        me.borrow().utxo_tree.selection().connect_changed(
            clone!(@weak me => move |_| {
                let me = me.borrow();
                me.utxo_remove_btn.set_sensitive(me.utxo_tree.selection().selected().is_some());
            }),
        );

        me.borrow().utxo_remove_btn.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let me = me.borrow();
            if let Some((outpoint, _, iter)) = Self::utxo_selection(&me.utxo_tree) {
                let utxo = doc
                    .borrow()
                    .utxo_by_outpoint(outpoint)
                    .expect("UTXO must be known since it is selected");
                let dlg = gtk::MessageDialog::new(
                    Some(&me.window),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::YesNo,
                    &format!("Please confirm deletion of {}", utxo)
                );
                if dlg.run() == gtk::ResponseType::Yes {
                    me.utxo_store.remove(&iter);
                    let _ = doc.borrow_mut().remove_utxo(utxo);
                    if let Some((generator, _, _)) = me.descriptor_selection() {
                        let descriptor_generator = doc
                            .borrow()
                            .descriptor_by_generator(&generator)
                            .expect("Descriptor must be known since it is selected");
                        doc.borrow().fill_utxo_store(&me.utxo_descr_store, Some(&descriptor_generator));
                    } else {
                        me.utxo_descr_store.clear();
                    }
                    me.utxo_descr_clear_btn.set_sensitive(me.utxo_descr_store.iter_first().is_some());
                }
                dlg.hide();
            }
        }));

        me.borrow().asset_tree.selection().connect_changed(
            clone!(@weak me, @strong doc => move |_| {
                let me = me.borrow();
                if let Some((id, _, _)) = me.asset_selection() {
                    me.asset_remove_btn.set_sensitive(true);
                    if let Some((asset, genesis)) = doc.borrow().asset_by_id(id) {
                        me.asset_id_display.set_text(&id.to_bech32_string());
                        me.asset_genesis_display.set_text(&genesis.to_bech32_string());
                        me.asset_contract_display.set_text(&asset.description().clone().unwrap_or_default());
                        me.asset_issued_display.set_text(&asset.accounting_supply(SupplyMeasure::KnownCirculating).to_string());
                        me.asset_total_display.set_text(&asset.accounting_supply(SupplyMeasure::IssueLimit).to_string());
                        me.asset_decimals_display.set_text(&asset.decimal_precision().to_string());

                        let png = qrcode_generator::to_png_to_vec(
                            genesis.to_bech32_string(),
                            QrCodeEcc::Low,
                            1024,
                        )
                        .ok();
                        let pixbuf = png
                            .and_then(|vec| {
                                let loader = PixbufLoader::new();
                                loader.write(&vec).ok()?;
                                loader.pixbuf()
                            }).and_then(|pixbuf| {
                                pixbuf.scale_simple(250, 250, InterpType::Bilinear)
                            });
                        me.asset_qr_image.set_from_pixbuf(pixbuf.as_ref());
                    }
                } else {
                    me.asset_remove_btn.set_sensitive(false);
                }
            }),
        );

        let tb: gtk::ToolButton = builder.object("assetCreate")?;
        tb.connect_clicked(clone!(@weak me, @strong doc => move |_| {
            let issue_dlg = AssetDlg::load_glade().expect("Must load");
            issue_dlg.run(doc.clone(), None, clone!(@weak me, @strong doc =>
                move |asset, genesis| {
                    let contract_id = genesis.contract_id();
                    let consignment = Consignment::with(genesis, none!(), none!(), none!());
                    let me = me.borrow();
                    me.asset_store.insert_with_values(
                        None,
                        &[
                            (0, &asset.ticker()),
                            (1, &asset.name()),
                            (2, &asset.known_filtered_accounting_value(|allocation| {
                                doc.borrow().is_outpoint_known(*allocation.outpoint())
                            })),
                            (3, &asset.accounting_supply(SupplyMeasure::KnownCirculating)),
                            (4, &1),
                            (5, &(!asset.known_inflation().is_empty())),
                            (6, &0),
                            (7, &contract_id.to_string())
                        ],
                    );
                    let _ = doc.borrow_mut().add_asset(consignment);
                }),
                || {},
            );
        }));

        me.borrow().asset_remove_btn.connect_clicked(
            clone!(@weak me, @strong doc => move |_| {
                let me = me.borrow();
                if let Some((contract_id, _, iter)) = me.asset_selection() {
                    let dlg = gtk::MessageDialog::new(
                        Some(&me.window),
                        gtk::DialogFlags::MODAL,
                        gtk::MessageType::Question,
                        gtk::ButtonsType::YesNo,
                        &format!(
                            "Please confirm deletion of the asset with id {}",
                            contract_id.to_bech32_string()
                        )
                    );
                    if dlg.run() == gtk::ResponseType::Yes {
                        me.asset_store.remove(&iter);
                        let _ = doc.borrow_mut().remove_asset(contract_id);
                    }
                    dlg.hide();
                    me.update_ui();
                }
            }),
        );

        for ctl in &[
            &me.borrow().asset_id_display,
            &me.borrow().asset_genesis_display,
            &me.borrow().asset_issued_display,
            &me.borrow().asset_total_display,
            &me.borrow().asset_decimals_display,
        ] {
            ctl.connect_icon_press(clone!(@weak ctl => move |_, _, _| {
                let val = ctl.text();
                gtk::Clipboard::get(&gdk::SELECTION_CLIPBOARD)
                    .set_text(&val);
            }));
        }

        let tb: gtk::Button = builder.object("save")?;
        tb.set_sensitive(needs_save);
        tb.connect_clicked(clone!(@strong doc, @weak tb => move |_| {
            let save_dlg = SaveDlg::load_glade().expect("Must load");
            let name = doc.borrow().name();
            save_dlg.run(name, clone!(@strong doc, @weak tb => move |path| {
                let mut path = path;
                path.set_extension("bpro");
                if doc.borrow_mut().save_as(path).is_ok() {
                    tb.set_sensitive(false);
                }
            }), || {})
        }));

        Some(me)
    }
}

impl BproWin {
    pub fn new(path: Option<PathBuf>) -> Option<Rc<RefCell<Self>>> {
        let doc = if let Some(path) = path {
            Some(Document::load(path).ok()?)
        } else {
            None
        };
        let me = Self::load_glade(doc)?;
        Some(me)
    }

    pub fn run(
        &self,
        on_open: impl Fn() + 'static,
        on_new: impl Fn() + 'static,
    ) {
        self.update_ui();

        self.new_btn.connect_clicked(move |_| on_new());
        self.open_btn.connect_clicked(move |_| on_open());

        self.window.show_all();
        gtk::main();
    }

    pub fn pubkey_selection(
        &self,
    ) -> Option<(String, gtk::TreeModel, gtk::TreeIter)> {
        self.pubkey_tree
            .selection()
            .selected()
            .and_then(|(model, iter)| {
                model
                    .value(&iter, 1)
                    .get::<String>()
                    .ok()
                    .map(|keyname| (keyname, model, iter))
            })
    }

    pub fn descriptor_selection(
        &self,
    ) -> Option<(String, gtk::TreeModel, gtk::TreeIter)> {
        self.descriptor_tree
            .selection()
            .selected()
            .and_then(|(model, iter)| {
                model
                    .value(&iter, 2)
                    .get::<String>()
                    .ok()
                    .map(|name| (name, model, iter))
            })
    }

    pub fn utxo_selection(
        utxo_tree: &gtk::TreeView,
    ) -> Option<(OutPoint, gtk::TreeModel, gtk::TreeIter)> {
        utxo_tree.selection().selected().and_then(|(model, iter)| {
            let txid = model
                .value(&iter, 0)
                .get::<String>()
                .ok()
                .map(|txid| Txid::from_str(&txid))
                .transpose()
                .ok()
                .flatten();
            let vout = model.value(&iter, 1).get::<u32>().ok();
            vout.and_then(|vout| {
                txid.map(|txid| (OutPoint { txid, vout }, model, iter))
            })
        })
    }

    pub fn asset_selection(
        &self,
    ) -> Option<(ContractId, gtk::TreeModel, gtk::TreeIter)> {
        self.asset_tree
            .selection()
            .selected()
            .and_then(|(model, iter)| {
                model
                    .value(&iter, 7)
                    .get::<String>()
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .map(|id| (id, model, iter))
            })
    }

    pub fn update_ui(&self) {}
}
