import { Component, OnInit } from '@angular/core';
import { NzModalRef } from 'ng-zorro-antd/modal';
import { FormBuilder, FormControl, FormGroup, Validators, ValidatorFn } from '@angular/forms';

@Component({
  selector: 'app-add-device',
  templateUrl: './add-device.component.html',
  styleUrls: ['./add-device.component.scss']
})
export class AddDeviceComponent implements OnInit {
  form = new FormGroup({
    "name": new FormControl(""),
    "ipv4": new FormControl("")
  });

  constructor(private modal: NzModalRef) {}

  ngOnInit(): void {
  }

  cancel(): void {
    this.modal.destroy(false);
  }

  add() {
    let data = this.form.value;
    data.name = data.name.trim();
    if (data.name === "") {
      delete data.name;
    }
    data.id = 0;

    fetch("/api/device", {
        method: "POST", body: JSON.stringify(data), headers: {
            "Content-Type": "application/json"
        },
    }).then(errors => {
      this.modal.destroy(true);
    })
  }
}
